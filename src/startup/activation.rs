use anyhow::Context as _;
use gpui::Window;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use wayland_backend::client::{Backend, ObjectId};
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle, delegate_noop,
    globals::{GlobalListContents, registry_queue_init},
    protocol::{wl_registry::WlRegistry, wl_surface::WlSurface},
};
use wayland_protocols::xdg::activation::v1::client::xdg_activation_v1::XdgActivationV1;

#[derive(Default)]
struct Activation;

pub(super) fn activate(window: &Window, token: Option<&str>) -> anyhow::Result<()> {
    let Some(token) = token else {
        window.activate_window();
        return Ok(());
    };
    let surface = HasWindowHandle::window_handle(window)?.as_raw();
    let display = window.display_handle()?.as_raw();
    let (RawWindowHandle::Wayland(surface), RawDisplayHandle::Wayland(display)) =
        (surface, display)
    else {
        window.activate_window();
        return Ok(());
    };
    // SAFETY: the window and gpui's display remain alive throughout this call.
    let backend = unsafe { Backend::from_foreign_display(display.display.as_ptr().cast()) };
    let connection = Connection::from_backend(backend);
    let (globals, queue) =
        registry_queue_init::<Activation>(&connection).context("activation: Wayland globals")?;
    let activation: XdgActivationV1 = globals
        .bind(&queue.handle(), 1..=1, ())
        .context("activation: xdg_activation_v1")?;
    // SAFETY: gpui's live surface on the borrowed connection.
    let id =
        unsafe { ObjectId::from_ptr(WlSurface::interface(), surface.surface.as_ptr().cast()) }?;
    let surface = WlSurface::from_id(&connection, id)?;
    activation.activate(token.to_owned(), &surface);
    activation.destroy();
    connection
        .flush()
        .context("activation: request did not reach compositor")?;
    log::debug!("startup: used desktop activation token");
    Ok(())
}

impl Dispatch<WlRegistry, GlobalListContents> for Activation {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

delegate_noop!(Activation: XdgActivationV1);
