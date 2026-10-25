use wayland_client::{
    Dispatch, QueueHandle, WEnum, delegate_noop, event_created_child,
    protocol::{
        wl_buffer::WlBuffer,
        wl_compositor::WlCompositor,
        wl_data_device::{self, WlDataDevice},
        wl_data_device_manager::WlDataDeviceManager,
        wl_data_offer::{self, WlDataOffer},
        wl_data_source::{self, WlDataSource},
        wl_keyboard::{self, WlKeyboard},
        wl_pointer::{self, WlPointer},
        wl_registry::WlRegistry,
        wl_seat::{self, WlSeat},
        wl_shm::WlShm,
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
    },
};

use super::*;

impl Dispatch<WlRegistry, wayland_client::globals::GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: wayland_client::protocol::wl_registry::Event,
        _: &wayland_client::globals::GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSeat, ()> for State {
    fn event(
        this: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(capabilities),
        } = event
        {
            if capabilities.contains(wl_seat::Capability::Pointer) && this.pointer.is_none() {
                this.pointer = Some(seat.get_pointer(qh, ()));
            }
            if capabilities.contains(wl_seat::Capability::Keyboard) && this.keyboard.is_none() {
                this.keyboard = Some(seat.get_keyboard(qh, ()));
            }
        }
    }
}

impl Dispatch<WlPointer, ()> for State {
    fn event(
        this: &mut Self,
        _: &WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_pointer::Event::Button {
            serial,
            button,
            state: WEnum::Value(wl_pointer::ButtonState::Pressed),
            ..
        } = event
        {
            if button == 272 {
                this.serial = Some(serial);
            }
            if button == 273 && this.dragging {
                this.cancelled = true;
                this.events.push(Event::Cancel);
            }
        }
    }
}

impl Dispatch<WlKeyboard, ()> for State {
    fn event(
        this: &mut Self,
        _: &WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_keyboard::Event::Key {
            key: 1,
            state: WEnum::Value(wl_keyboard::KeyState::Pressed),
            ..
        } = event
            && this.dragging
        {
            this.cancelled = true;
            this.events.push(Event::Cancel);
        }
    }
}

impl Dispatch<WlDataDevice, ()> for State {
    fn event(
        this: &mut Self,
        _: &WlDataDevice,
        event: wl_data_device::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_device::Event::Enter {
                serial,
                surface,
                x,
                y,
                id,
            } if this.dragging => {
                this.accepted = false;
                this.offer = id.filter(|offer| this.compatible.contains(&offer.id()));
                if this.offer.is_some() {
                    let position = point(px(x as f32), px(y as f32));
                    this.entered = Some((surface.id(), position, serial));
                    this.events.push(Event::Hover(surface.id(), position));
                }
            }
            wl_data_device::Event::Motion { x, y, .. } if this.dragging => {
                if let Some((surface, position, _)) = &mut this.entered {
                    *position = point(px(x as f32), px(y as f32));
                    this.events.push(Event::Hover(surface.clone(), *position));
                }
            }
            wl_data_device::Event::Leave if this.dragging => {
                this.entered = None;
                this.offer = None;
                this.accepted = false;
                this.events.push(Event::Leave);
            }
            wl_data_device::Event::Drop if this.dragging && this.accepted => {
                this.events.push(Event::Drop);
                if let Some(offer) = this.offer.take() {
                    offer.finish();
                    offer.destroy();
                }
            }
            _ => {}
        }
    }
    event_created_child!(State, WlDataDevice, [wl_data_device::EVT_DATA_OFFER_OPCODE => (WlDataOffer, ())]);
}

impl Dispatch<WlDataOffer, ()> for State {
    fn event(
        this: &mut Self,
        offer: &WlDataOffer,
        event: wl_data_offer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_data_offer::Event::Offer { mime_type } = event
            && mime_type == MIME
        {
            this.compatible.insert(offer.id());
        }
    }
}

impl Dispatch<WlDataSource, ()> for State {
    fn event(
        this: &mut Self,
        _: &WlDataSource,
        event: wl_data_source::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_source::Event::Send { fd, mime_type } if mime_type == MIME => {
                use std::io::Write;
                let mut file = std::fs::File::from(fd);
                let _ = file.write_all(b"ELYTerminal live session\n");
            }
            wl_data_source::Event::DndFinished if this.dragging => {
                this.events.push(Event::End { cancelled: true });
            }
            wl_data_source::Event::Cancelled if this.dragging => {
                this.events.push(Event::End {
                    cancelled: this.cancelled,
                });
            }
            _ => {}
        }
    }
}

delegate_noop!(State: ignore WlCompositor);
delegate_noop!(State: ignore WlSurface);
delegate_noop!(State: ignore WlShm);
delegate_noop!(State: ignore WlShmPool);
delegate_noop!(State: ignore WlBuffer);
delegate_noop!(State: ignore WlDataDeviceManager);
