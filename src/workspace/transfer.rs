use super::Workspace;
use crate::{
    app,
    sessions::{self, Session},
    term::TerminalView,
};
use anyhow::Context as _;
use gpui::{Context, Entity, EntityId, Pixels, Point, Subscription, Window, WindowHandle};

mod drag;
mod list;
#[cfg(target_os = "linux")]
mod native;
pub(super) use drag::{cancel, handlers};
pub(super) use list::{Layout, list};

impl Workspace {
    /// Moves the existing terminal into another window at the requested list position.
    pub fn transfer_session(
        &mut self,
        id: EntityId,
        target: WindowHandle<Self>,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let from = self
            .sessions
            .iter()
            .position(|session| session.terminal.entity_id() == id)
            .context("app: the session to transfer no longer exists")?;
        self.end_rename(id, true, window, cx);
        let pid = self.sessions[from].terminal.read(cx).pid();
        self.sessions[from]
            .terminal
            .update(cx, |view, _| view.set_front(false, window));
        let mut moved = Some(self.sessions.remove(from));
        if let Err(error) = target.update(cx, |destination, window, cx| {
            destination.adopt(
                moved.take().expect("transferring session"),
                index,
                window,
                cx,
            );
            window.activate_window();
        }) {
            if let Some(session) = moved {
                self.sessions.insert(from, session);
                self.select(self.active, window, cx);
            }
            return Err(error).context("app: session transfer destination closed");
        }
        if self.sessions.is_empty() {
            window.remove_window();
        } else {
            self.active = sessions::after_close(self.active, from, self.sessions.len());
            self.select(self.active, window, cx);
        }
        log::info!("app: session {pid} inserted at {index} in another window without restarting");
        Ok(())
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
        self.sessions[index]
            .terminal
            .update(cx, |view, _| view.set_front(false, window));
        let mut moved = Some(self.sessions.remove(index));
        if let Err(error) = target.update(cx, |destination, window, cx| {
            destination.adopt(
                moved.take().expect("a session being transferred"),
                0,
                window,
                cx,
            );
            window.activate_window();
        }) {
            if let Some(session) = moved {
                self.sessions.insert(index, session);
                self.select(self.active, window, cx);
            }
            let _ = target.update(cx, |_, window, _| window.remove_window());
            return Err(error).context("app: could not populate detached window");
        }
        if self.sessions.is_empty() {
            window.remove_window();
        } else {
            self.active = sessions::after_close(self.active, index, self.sessions.len());
            self.select(self.active, window, cx);
        }
        log::info!("app: session {pid} moved to a new window without restarting");
        Ok(target)
    }

    fn adopt(
        &mut self,
        mut session: Session,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        session.editing = None;
        session
            .terminal
            .update(cx, |terminal, cx| terminal.move_to_window(window, cx));
        session._events = Self::watch_terminal(&session.terminal, window, cx);
        let index = index.min(self.sessions.len());
        if !self.sessions.is_empty() && index <= self.active {
            self.active += 1;
        }
        self.sessions.insert(index, session);
        self.select(index, window, cx);
    }
}
