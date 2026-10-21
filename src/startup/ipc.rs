use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt},
            net::{UnixListener, UnixStream},
        },
    },
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context as _, ensure};
use futures::channel::mpsc::{UnboundedReceiver, unbounded};
use serde::{Serialize, de::DeserializeOwned};

use super::Request;

const WAIT: Duration = Duration::from_secs(20);
const LIMIT: usize = 1024 * 1024;

pub(super) enum Instance {
    Owner(Owner),
    Forwarded,
}

pub(super) struct Owner {
    _lock: File,
    listener: UnixListener,
    socket: PathBuf,
    stopped: Arc<AtomicBool>,
}

pub(super) struct Incoming {
    pub request: Request,
    pub reply: mpsc::SyncSender<Result<(), String>>,
}

pub(super) fn claim(request: &Request) -> anyhow::Result<Instance> {
    let runtime =
        std::env::var_os("XDG_RUNTIME_DIR").context("startup: XDG_RUNTIME_DIR is not set")?;
    let directory = PathBuf::from(runtime).join("elyterminal");
    private_directory(&directory)?;
    let display = std::env::var_os("WAYLAND_DISPLAY")
        .or_else(|| std::env::var_os("DISPLAY"))
        .context("startup: no graphical display")?;
    use std::os::unix::ffi::OsStrExt;
    let hash = display
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ *byte as u64).wrapping_mul(0x100000001b3)
        });
    acquire(&directory, &format!("{hash:016x}"), request)
}

fn private_directory(path: &Path) -> anyhow::Result<()> {
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error).context("startup: cannot create runtime directory"),
    }
    let metadata = fs::symlink_metadata(path)?;
    // SAFETY: geteuid reads this process's effective user id.
    let uid = unsafe { libc::geteuid() };
    ensure!(
        metadata.is_dir() && metadata.uid() == uid && metadata.mode() & 0o077 == 0,
        "startup: runtime directory must belong to this user with mode 0700: {}",
        path.display()
    );
    Ok(())
}

fn acquire(directory: &Path, name: &str, request: &Request) -> anyhow::Result<Instance> {
    let socket = directory.join(format!("{name}.sock"));
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(directory.join(format!("{name}.lock")))
        .context("startup: cannot open instance lock")?;
    let deadline = Instant::now() + WAIT;
    loop {
        // SAFETY: the file descriptor remains open for the owner's entire lifetime.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            match fs::symlink_metadata(&socket) {
                Ok(metadata) => {
                    ensure!(
                        metadata.file_type().is_socket(),
                        "startup: unexpected file at {}",
                        socket.display()
                    );
                    fs::remove_file(&socket).context("startup: cannot remove stale socket")?;
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error).context("startup: cannot inspect instance socket"),
            }
            let listener =
                UnixListener::bind(&socket).context("startup: cannot bind instance socket")?;
            listener.set_nonblocking(true)?;
            log::info!("startup: instance owner on {}", socket.display());
            return Ok(Instance::Owner(Owner {
                _lock: lock,
                listener,
                socket,
                stopped: Arc::default(),
            }));
        }
        let error = io::Error::last_os_error();
        ensure!(
            error.kind() == io::ErrorKind::WouldBlock,
            "startup: cannot acquire instance lock: {error}"
        );
        match UnixStream::connect(&socket) {
            Ok(mut stream) => {
                stream.set_read_timeout(Some(WAIT))?;
                stream.set_write_timeout(Some(WAIT))?;
                send(&mut stream, request)
                    .context("startup: request transmission failed; it will not be retried")?;
                let result: Result<(), String> = receive(&mut stream)
                    .context("startup: no completion reply; request will not be retried")?;
                result.map_err(anyhow::Error::msg)?;
                log::info!("startup: existing window accepted the request");
                return Ok(Instance::Forwarded);
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                ) && Instant::now() < deadline =>
            {
                thread::sleep(Duration::from_millis(20));
            }
            Err(error) => {
                return Err(error).context("startup: cannot connect to existing instance");
            }
        }
    }
}

impl Owner {
    pub fn listen(&self) -> anyhow::Result<UnboundedReceiver<Incoming>> {
        let listener = self.listener.try_clone()?;
        let stopped = self.stopped.clone();
        let (sender, receiver) = unbounded();
        thread::Builder::new()
            .name("ely-launches".into())
            .spawn(move || {
                while !stopped.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            let handled = (|| {
                                stream.set_read_timeout(Some(Duration::from_secs(2)))?;
                                stream.set_write_timeout(Some(Duration::from_secs(2)))?;
                                let request: Request = receive(&mut stream)?;
                                request.validate()?;
                                let (reply, result) = mpsc::sync_channel(1);
                                sender
                                    .unbounded_send(Incoming { request, reply })
                                    .context("startup: application stopped accepting launches")?;
                                let result = result
                                    .recv_timeout(WAIT)
                                    .context("startup: application did not complete launch")?;
                                anyhow::Ok(result)
                            })()
                            .unwrap_or_else(|error| Err(format!("{error:#}")));
                            if let Err(error) = &handled {
                                log::error!("startup: rejected launch: {error}");
                            }
                            if let Err(error) = send(&mut stream, &handled) {
                                log::warn!("startup: reply failed: {error:#}");
                            }
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(10))
                        }
                        Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                        Err(error) => {
                            log::error!("startup: listener failed: {error}");
                            break;
                        }
                    }
                }
            })
            .context("startup: cannot start launch listener")?;
        Ok(receiver)
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        if let Err(error) = fs::remove_file(&self.socket) {
            log::warn!("startup: socket cleanup failed: {error}");
        }
    }
}

fn send(stream: &mut UnixStream, value: &impl Serialize) -> anyhow::Result<()> {
    let bytes = serde_json::to_vec(value)?;
    ensure!(
        bytes.len() <= LIMIT,
        "startup: launch message exceeds 1 MiB"
    );
    stream.write_all(&(bytes.len() as u32).to_be_bytes())?;
    stream.write_all(&bytes)?;
    Ok(())
}

fn receive<T: DeserializeOwned>(stream: &mut UnixStream) -> anyhow::Result<T> {
    let mut header = [0; 4];
    stream.read_exact(&mut header)?;
    let length = u32::from_be_bytes(header) as usize;
    ensure!(length <= LIMIT, "startup: launch message exceeds 1 MiB");
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes)?;
    serde_json::from_slice(&bytes).context("startup: invalid launch message")
}

#[cfg(test)]
mod tests;
