use std::time::Duration;

use anyhow::Context as _;
use gpui::{
    App, Context, DispatchPhase, Entity, EntityId, InteractiveElement, IntoElement, MouseButton,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Size, Styled, Subscription, Window,
    WindowHandle, canvas, div, point, px,
};

use super::Workspace;
use crate::{
    app,
    sessions::{self, Session},
    term::TerminalView,
};

pub(super) struct SessionDrag {
    id: EntityId,
    origin: Point<Pixels>,
}

pub(super) fn draggable(
    row: impl IntoElement,
    workspace: &Entity<Workspace>,
    session: &Session,
    cx: &App,
) -> impl IntoElement {
    let workspace = workspace.clone();
    let id = session.terminal.entity_id();
    let pid = session.terminal.read(cx).pid() as usize;
    let editing = session.editing.is_some();
    div().id(("session-drag", pid)).child(row).on_mouse_down(
        MouseButton::Left,
        move |event, _, cx| {
            if !editing && event.click_count == 1 {
                workspace.update(cx, |this, _| {
                    this.session_drag = Some(SessionDrag {
                        id,
                        origin: event.position,
                    });
                });
            }
        },
    )
}

pub(super) fn handlers(workspace: &Entity<Workspace>) -> impl IntoElement {
    let workspace = workspace.clone();
    canvas(
        |_, _, _| (),
        move |_, (), window, _| {
            let moving = workspace.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                if phase == DispatchPhase::Capture {
                    moving.update(cx, |this, cx| this.drag_session(event, window, cx));
                }
            });
            let released = workspace.clone();
            window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                if phase == DispatchPhase::Capture {
                    released.update(cx, |this, _| this.session_drag = None);
                }
            });
        },
    )
    .absolute()
    .size_full()
}

fn crossed(origin: Point<Pixels>, position: Point<Pixels>, size: Size<Pixels>) -> bool {
    let distance = (position.x - origin.x)
        .abs()
        .max((position.y - origin.y).abs());
    let margin = px(12.0);
    distance > px(8.0)
        && (position.x < -margin
            || position.y < -margin
            || position.x > size.width + margin
            || position.y > size.height + margin)
}

impl Workspace {
    fn drag_session(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = &self.session_drag else {
            return;
        };
        if !event.dragging()
            || !self
                .sessions
                .iter()
                .any(|session| session.terminal.entity_id() == drag.id)
        {
            self.session_drag = None;
            return;
        }
        if !crossed(drag.origin, event.position, window.viewport_size()) {
            return;
        }
        let id = self.session_drag.take().expect("a held session drag").id;
        cx.stop_propagation();
        if self.sessions.len() == 1 {
            window.start_window_move();
            log::info!("app: dragging the only session moves its window");
            return;
        }
        let origin =
            window.window_bounds().get_bounds().origin + event.position - point(px(100.), px(20.));
        match self.detach_session(id, origin, window, cx) {
            Ok(target) => {
                cx.spawn(async move |_, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(50))
                        .await;
                    let _ = target.update(cx, |_, window, _| window.start_window_move());
                })
                .detach();
            }
            Err(error) => log::error!("app: session stayed in its window: {error:#}"),
        }
    }

    pub(super) fn watch_terminal(
        terminal: &Entity<TerminalView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        let id = terminal.entity_id();
        cx.subscribe_in(terminal, window, move |this, _, event, window, cx| {
            this.session_event(id, event, window, cx)
        })
    }

    /// Moves a live session into a new window without replacing its terminal or pty.
    pub fn detach_session(
        &mut self,
        id: EntityId,
        origin: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<WindowHandle<Self>> {
        let index = self
            .sessions
            .iter()
            .position(|session| session.terminal.entity_id() == id)
            .context("app: the session to detach no longer exists")?;
        self.end_rename(id, true, window, cx);
        let target = app::empty_window(self.sessions[index].folder.clone(), origin, cx)?;
        let pid = self.sessions[index].terminal.read(cx).pid();
        let mut moved = Some(self.sessions.remove(index));
        if let Err(error) = target.update(cx, |destination, window, cx| {
            destination.adopt(
                moved.take().expect("a session being transferred"),
                window,
                cx,
            );
            window.activate_window();
        }) {
            if let Some(session) = moved {
                self.sessions.insert(index, session);
            }
            let _ = target.update(cx, |_, window, _| window.remove_window());
            return Err(error).context("app: could not populate detached window");
        }
        self.session_drag = None;
        if self.sessions.is_empty() {
            window.remove_window();
        } else {
            self.active = sessions::after_close(self.active, index, self.sessions.len());
            self.select(self.active, window, cx);
        }
        log::info!("app: session {pid} moved to a new window without restarting");
        Ok(target)
    }

    fn adopt(&mut self, mut session: Session, window: &mut Window, cx: &mut Context<Self>) {
        session.editing = None;
        session
            .terminal
            .update(cx, |terminal, cx| terminal.move_to_window(window, cx));
        session._events = Self::watch_terminal(&session.terminal, window, cx);
        self.sessions.push(session);
        self.select(self.sessions.len() - 1, window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::size;

    #[test]
    fn a_drag_must_cross_the_window_edge_with_a_margin() {
        let origin = point(px(40.), px(70.));
        let room = size(px(1200.), px(760.));
        for inside in [
            point(px(900.), px(200.)),
            point(px(-5.), px(70.)),
            point(px(1205.), px(70.)),
        ] {
            assert!(!crossed(origin, inside, room));
        }
        for outside in [
            point(px(-20.), px(70.)),
            point(px(1220.), px(70.)),
            point(px(40.), px(-20.)),
            point(px(40.), px(780.)),
        ] {
            assert!(crossed(origin, outside, room));
        }
        assert!(!crossed(
            point(px(-15.), px(70.)),
            point(px(-16.), px(70.)),
            room
        ));
    }
}
