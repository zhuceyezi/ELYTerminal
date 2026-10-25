use anyhow::Context as _;
use std::{
    fs::{self, File},
    os::fd::AsRawFd,
};

const LEFT: usize = 272;
const RIGHT: usize = 273;
const ESC: usize = 1;
const LENGTH: usize = 96;

pub(super) struct Buttons(Vec<File>);

#[derive(Default)]
pub(super) struct Pressed {
    pub left: bool,
    pub cancel: bool,
}

fn bit(bytes: &[u8], code: usize) -> bool {
    bytes[code / 8] & (1 << (code % 8)) != 0
}

fn query(file: &File, number: u32) -> std::io::Result<[u8; LENGTH]> {
    let mut bytes = [0; LENGTH];
    let request = (2u32 << 30) | ((LENGTH as u32) << 16) | ((b'E' as u32) << 8) | number;
    // SAFETY: EVIOCGKEY and EVIOCGBIT write at most the supplied bitmap length.
    let result = unsafe {
        libc::ioctl(
            file.as_raw_fd(),
            request as libc::c_ulong,
            bytes.as_mut_ptr(),
        )
    };
    if result < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(bytes)
}

impl Buttons {
    pub fn open() -> anyhow::Result<Self> {
        let mut devices = Vec::new();
        let mut mouse = false;
        for entry in fs::read_dir("/dev/input").context("session drag: pointer button state")? {
            let entry = entry?;
            if !entry.file_name().to_string_lossy().starts_with("event") {
                continue;
            }
            let Ok(file) = File::open(entry.path()) else {
                continue;
            };
            let Ok(capabilities) = query(&file, 0x21) else {
                continue;
            };
            if bit(&capabilities, LEFT) || bit(&capabilities, RIGHT) {
                mouse = true;
            }
            if bit(&capabilities, LEFT) || bit(&capabilities, RIGHT) || bit(&capabilities, ESC) {
                devices.push(file);
            }
        }
        anyhow::ensure!(
            mouse,
            "session drag: no readable pointer button state for native cancellation"
        );
        Ok(Self(devices))
    }

    /// Reads current button bits during this drag only; never reads the input event stream.
    pub fn pressed(&self) -> Pressed {
        let mut pressed = Pressed::default();
        for file in &self.0 {
            if let Ok(keys) = query(file, 0x18) {
                pressed.left |= bit(&keys, LEFT);
                pressed.cancel |= bit(&keys, RIGHT) || bit(&keys, ESC);
            }
        }
        pressed
    }
}
