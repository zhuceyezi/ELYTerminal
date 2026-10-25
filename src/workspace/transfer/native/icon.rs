use super::*;
use resvg::{tiny_skia, usvg};
use std::{fs::File, io::Write, os::fd::FromRawFd};
use wayland_client::{
    QueueHandle,
    protocol::{wl_buffer::WlBuffer, wl_shm_pool::WlShmPool},
};

pub(super) struct Icon {
    pub surface: WlSurface,
    buffer: WlBuffer,
    pool: WlShmPool,
    _file: File,
}

impl Icon {
    pub fn new(
        title: &str,
        offset: Point<Pixels>,
        colors: [gpui::Hsla; 3],
        compositor: &WlCompositor,
        shm: &WlShm,
        qh: &QueueHandle<State>,
    ) -> anyhow::Result<Self> {
        let title: String = title.chars().take(34).collect();
        let title = title
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        let hex = |color: gpui::Hsla| {
            let color: gpui::Rgba = color.into();
            format!(
                "#{:02x}{:02x}{:02x}",
                (color.r * 255.) as u8,
                (color.g * 255.) as u8,
                (color.b * 255.) as u8
            )
        };
        let [bg, fg, border] = colors.map(hex);
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="280" height="44"><rect x="1" y="1" width="278" height="42" rx="9" fill="{bg}" stroke="{border}"/><text x="14" y="28" font-family="sans-serif" font-size="14" fill="{fg}">{title}</text></svg>"#
        );
        let mut options = usvg::Options::default();
        options.fontdb_mut().load_system_fonts();
        let tree = usvg::Tree::from_str(&svg, &options)?;
        let mut pixmap = tiny_skia::Pixmap::new(560, 88).context("session drag: label image")?;
        resvg::render(
            &tree,
            tiny_skia::Transform::from_scale(2., 2.),
            &mut pixmap.as_mut(),
        );
        let mut bytes = pixmap.take();
        for pixel in bytes.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        // SAFETY: memfd_create returns an owned descriptor or -1.
        let fd = unsafe { libc::memfd_create(c"ely-session-label".as_ptr(), libc::MFD_CLOEXEC) };
        anyhow::ensure!(
            fd >= 0,
            "session drag: label buffer: {}",
            std::io::Error::last_os_error()
        );
        // SAFETY: this descriptor is new and owned by this icon.
        let mut file = unsafe { File::from_raw_fd(fd) };
        file.write_all(&bytes)?;
        let pool = shm.create_pool(file.as_fd(), bytes.len() as i32, qh, ());
        let buffer = pool.create_buffer(0, 560, 88, 560 * 4, Format::Argb8888, qh, ());
        let surface = compositor.create_surface(qh, ());
        surface.set_buffer_scale(2);
        surface.attach(
            Some(&buffer),
            -(f32::from(offset.x).clamp(0., 260.) as i32),
            -(f32::from(offset.y).clamp(0., 40.) as i32),
        );
        surface.damage(0, 0, 280, 44);
        surface.commit();
        Ok(Self {
            surface,
            buffer,
            pool,
            _file: file,
        })
    }
}

impl Drop for Icon {
    fn drop(&mut self) {
        self.surface.destroy();
        self.buffer.destroy();
        self.pool.destroy();
    }
}
