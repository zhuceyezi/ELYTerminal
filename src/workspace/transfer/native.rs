use std::{collections::HashSet, mem::ManuallyDrop, os::fd::AsFd};

use anyhow::Context as _;
use gpui::{Pixels, Point, Window, point, px};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use wayland_backend::client::{Backend, ObjectId};
use wayland_client::{
    Connection, EventQueue, Proxy,
    globals::registry_queue_init,
    protocol::{
        wl_compositor::WlCompositor,
        wl_data_device::WlDataDevice,
        wl_data_device_manager::{DndAction, WlDataDeviceManager},
        wl_data_offer::WlDataOffer,
        wl_data_source::WlDataSource,
        wl_keyboard::WlKeyboard,
        wl_pointer::WlPointer,
        wl_seat::WlSeat,
        wl_shm::{Format, WlShm},
        wl_surface::WlSurface,
    },
};

mod buttons;
mod events;
mod icon;

const MIME: &str = "application/x-elyterminal-live-session";

pub(super) enum Event {
    Hover(ObjectId, Point<Pixels>),
    Leave,
    Drop,
    End { cancelled: bool },
    Cancel,
}

#[derive(Default)]
struct State {
    serial: Option<u32>,
    pointer: Option<WlPointer>,
    keyboard: Option<WlKeyboard>,
    compatible: HashSet<ObjectId>,
    offer: Option<WlDataOffer>,
    entered: Option<(ObjectId, Point<Pixels>, u32)>,
    events: Vec<Event>,
    dragging: bool,
    cancelled: bool,
    accepted: bool,
}

pub(super) struct Native {
    connection: ManuallyDrop<Connection>,
    queue: EventQueue<State>,
    state: State,
    manager: WlDataDeviceManager,
    device: WlDataDevice,
    compositor: WlCompositor,
    shm: WlShm,
    source: Option<WlDataSource>,
    buttons: Option<buttons::Buttons>,
    icon: Option<icon::Icon>,
}

impl Native {
    pub fn new(window: &Window) -> anyhow::Result<Option<Self>> {
        let RawDisplayHandle::Wayland(display) = window.display_handle()?.as_raw() else {
            return Ok(None);
        };
        // SAFETY: gpui owns this display for the lifetime of the application.
        let backend = unsafe { Backend::from_foreign_display(display.display.as_ptr().cast()) };
        let connection = Connection::from_backend(backend);
        let (globals, mut queue) = registry_queue_init::<State>(&connection)?;
        let qh = queue.handle();
        let manager: WlDataDeviceManager = globals.bind(&qh, 3..=3, ())?;
        let seat: WlSeat = globals.bind(&qh, 5..=5, ())?;
        let compositor: WlCompositor = globals.bind(&qh, 4..=4, ())?;
        let shm: WlShm = globals.bind(&qh, 1..=1, ())?;
        let device = manager.get_data_device(&seat, &qh, ());
        let mut state = State::default();
        queue.roundtrip(&mut state)?;
        log::info!("session drag: Wayland native transport ready");
        Ok(Some(Self {
            connection: ManuallyDrop::new(connection),
            queue,
            state,
            manager,
            device,
            compositor,
            shm,
            source: None,
            buttons: None,
            icon: None,
        }))
    }

    pub fn surface(window: &Window) -> anyhow::Result<ObjectId> {
        let RawWindowHandle::Wayland(surface) = HasWindowHandle::window_handle(window)?.as_raw()
        else {
            anyhow::bail!("session drag: window is not on Wayland");
        };
        // SAFETY: this is gpui's live wl_surface on the borrowed display.
        unsafe { ObjectId::from_ptr(WlSurface::interface(), surface.surface.as_ptr().cast()) }
            .context("session drag: window surface")
    }

    pub fn start(
        &mut self,
        window: &Window,
        title: &str,
        offset: Point<Pixels>,
        colors: [gpui::Hsla; 3],
    ) -> anyhow::Result<()> {
        self.queue.dispatch_pending(&mut self.state)?;
        let serial = self
            .state
            .serial
            .context("session drag: no left-button press serial")?;
        let surface = WlSurface::from_id(&self.connection, Self::surface(window)?)?;
        let buttons = buttons::Buttons::open()?;
        anyhow::ensure!(
            buttons.pressed().left,
            "session drag: left button was already released"
        );
        let source = self.manager.create_data_source(&self.queue.handle(), ());
        source.offer(MIME.into());
        source.set_actions(DndAction::Move);
        let icon = icon::Icon::new(
            title,
            offset,
            colors,
            &self.compositor,
            &self.shm,
            &self.queue.handle(),
        )?;
        self.buttons = Some(buttons);
        self.state.compatible.clear();
        self.state.cancelled = false;
        self.state.accepted = false;
        self.state.dragging = true;
        self.state.entered = None;
        self.state.events.clear();
        self.device
            .start_drag(Some(&source), &surface, Some(&icon.surface), serial);
        self.source = Some(source);
        self.icon = Some(icon);
        self.connection.flush()?;
        log::info!("session drag: label lifted; terminal stays in its source window");
        Ok(())
    }

    pub fn poll(&mut self) -> anyhow::Result<Vec<Event>> {
        if self.state.dragging
            && self
                .buttons
                .as_ref()
                .is_some_and(|buttons| buttons.pressed().cancel)
        {
            self.state.cancelled = true;
            self.state.events.push(Event::Cancel);
        }
        self.queue.dispatch_pending(&mut self.state)?;
        if self
            .buttons
            .as_ref()
            .is_some_and(|buttons| buttons.pressed().left)
        {
            for event in &mut self.state.events {
                if matches!(event, Event::End { cancelled: false }) {
                    *event = Event::Cancel;
                }
            }
        }
        self.connection.flush()?;
        Ok(std::mem::take(&mut self.state.events))
    }

    pub fn accept(&mut self, accepted: bool) {
        self.state.accepted = accepted;
        if let (Some(offer), Some((_, _, serial))) = (&self.state.offer, &self.state.entered) {
            offer.accept(*serial, accepted.then(|| MIME.into()));
            let action = if accepted {
                DndAction::Move
            } else {
                DndAction::empty()
            };
            offer.set_actions(action, action);
        }
    }

    pub fn finish(&mut self) {
        self.buttons.take();
        self.state.dragging = false;
        self.state.entered = None;
        self.state.offer = None;
        self.state.events.clear();
        if let Some(source) = self.source.take() {
            source.destroy();
        }
        self.icon.take();
        let _ = self.connection.flush();
    }
}

impl Drop for Native {
    fn drop(&mut self) {
        // The app owns this borrowed display; its proxies must not outlive gpui.
        self.finish();
    }
}
