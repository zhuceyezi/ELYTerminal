use std::sync::atomic::AtomicUsize;

use super::*;
use futures::{StreamExt, executor::block_on};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "ely-ipc-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        private_directory(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn request() -> Request {
    Request::parse(Vec::new(), std::env::temp_dir()).unwrap()
}
fn owner(directory: &Path) -> Owner {
    match acquire(directory, "test", &request()).unwrap() {
        Instance::Owner(owner) => owner,
        Instance::Forwarded => panic!("first invocation must own the instance"),
    }
}

#[test]
fn one_owner_accepts_concurrent_launches_exactly_once() {
    let directory = Directory::new();
    let owner = owner(&directory.0);
    let mut launches = owner.listen().unwrap();
    thread::scope(|scope| {
        let callers: Vec<_> = (0..8)
            .map(|n| {
                let path = &directory.0;
                scope.spawn(move || {
                    let mut request = request();
                    request.command = vec![format!("request-{n}")];
                    assert!(matches!(
                        acquire(path, "test", &request).unwrap(),
                        Instance::Forwarded
                    ));
                })
            })
            .collect();
        let mut received = Vec::new();
        for _ in 0..8 {
            let launch = block_on(launches.next()).unwrap();
            received.push(launch.request.command[0].clone());
            launch.reply.send(Ok(())).unwrap();
        }
        received.sort();
        received.dedup();
        assert_eq!(received.len(), 8);
        for caller in callers {
            caller.join().unwrap();
        }
    });
}

#[test]
fn failed_session_creation_is_reported_to_the_caller() {
    let directory = Directory::new();
    let owner = owner(&directory.0);
    let mut launches = owner.listen().unwrap();
    thread::scope(|scope| {
        let caller = scope.spawn(|| acquire(&directory.0, "test", &request()));
        let launch = block_on(launches.next()).unwrap();
        launch
            .reply
            .send(Err("pty: program could not start".into()))
            .unwrap();
        assert!(
            caller
                .join()
                .unwrap()
                .err()
                .unwrap()
                .to_string()
                .contains("program could not start")
        );
    });
}

#[test]
fn stale_sockets_are_recovered_only_while_holding_the_lock() {
    let directory = Directory::new();
    let socket = directory.0.join("test.sock");
    drop(UnixListener::bind(&socket).unwrap());
    assert!(socket.exists());
    let first = owner(&directory.0);
    drop(first);
    assert!(!socket.exists());
    let second = owner(&directory.0);
    assert!(socket.exists());
    drop(second);
}

#[test]
fn oversized_frames_and_unexpected_files_are_rejected() {
    let (mut writer, mut reader) = UnixStream::pair().unwrap();
    writer
        .write_all(&((LIMIT + 1) as u32).to_be_bytes())
        .unwrap();
    assert!(receive::<Request>(&mut reader).is_err());
    let directory = Directory::new();
    fs::write(directory.0.join("test.sock"), "leave this file alone").unwrap();
    assert!(acquire(&directory.0, "test", &request()).is_err());
    assert_eq!(
        fs::read_to_string(directory.0.join("test.sock")).unwrap(),
        "leave this file alone"
    );
}

#[test]
fn insecure_runtime_directories_are_rejected() {
    use std::os::unix::fs::PermissionsExt;
    let directory = Directory::new();
    fs::set_permissions(&directory.0, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(private_directory(&directory.0).is_err());
}
