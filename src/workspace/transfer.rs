use super::Workspace;
use crate::{
    app,
    sessions::{self, Session},
    term::TerminalView,
};
use anyhow::Context as _;
use gpui::{
    Context, Entity, EntityId, Focusable, Pixels, Point, Subscription, Window, WindowHandle,
};

mod drag;
mod list;
#[cfg(target_os = "linux")]
mod native;
pub(super) use drag::{cancel, handlers};
pub(super) use list::{Layout, list};

impl Workspace {
    /// Moves a whole session, keeping its panes and layout.
    pub fn transfer_session(
        &mut self,
        id: EntityId,
        target: WindowHandle<Self>,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.leaving.is_none(),
            "app: finish the pending close first"
        );
        anyhow::ensure!(
            target != window.window_handle().downcast::<Self>().unwrap(),
            "app: use reorder inside the same window"
        );
        let from = self
            .sessions
            .iter()
            .position(|session| session.id == id)
            .context("app: the session to transfer no longer exists")?;
        self.end_rename(id, true, window, cx);
        for pane in &self.sessions[from].panes {
            pane.terminal
                .update(cx, |view, _| view.set_front(false, window));
        }
        let group = self.sessions[from].group.and_then(|id| {
            self.groups
                .iter()
                .find(|g| g.id == id)
                .map(|g| g.name.clone())
        });
        let mut moved = Some(self.sessions.remove(from));
        if let Err(error) = target
            .update(cx, |destination, window, cx| {
                anyhow::ensure!(
                    destination.leaving.is_none(),
                    "app: destination awaits a close decision"
                );
                destination.adopt_transferred(
                    moved.take().expect("transferring session"),
                    index,
                    group,
                    window,
                    cx,
                );
                window.activate_window();
                Ok(())
            })
            .and_then(|result| result)
        {
            if let Some(session) = moved {
                self.sessions.insert(from, session);
                self.select(self.active, window, cx);
            }
            return Err(error).context("app: session transfer destination closed");
        }
        self.prune_groups(window, cx);
        self.keep(cx);
        if self.sessions.is_empty() {
            window.remove_window();
        } else {
            self.active = sessions::after_close(self.active, from, self.sessions.len());
            self.select(self.active, window, cx);
        }
        log::info!("app: session {id:?} inserted at {index} in another window without restarting");
        Ok(())
    }

    pub(super) fn watch_terminal(
        terminal: &Entity<TerminalView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> [Subscription; 2] {
        let id = terminal.entity_id();
        let events = cx.subscribe_in(terminal, window, move |this, _, event, window, cx| {
            this.session_event(id, event, window, cx)
        });
        let focus = terminal.focus_handle(cx);
        let focused = cx.on_focus(&focus, window, move |this, window, cx| {
            this.pane_focused(id, window, cx)
        });
        [events, focused]
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
            .position(|session| session.id == id)
            .context("app: the session to detach no longer exists")?;
        anyhow::ensure!(
            self.leaving.is_none(),
            "app: finish the pending close first"
        );
        let target = app::empty_window(self.sessions[index].pane().folder.clone(), origin, cx)?;
        if let Err(error) = self.transfer_session(id, target, 0, window, cx) {
            let _ = target.update(cx, |_, window, _| window.remove_window());
            return Err(error).context("app: could not populate detached window");
        }
        Ok(target)
    }

    fn adopt_transferred(
        &mut self,
        mut session: Session,
        index: usize,
        group: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        session.editing = None;
        session.group = group.map(|name| {
            if let Some(group) = self.groups.iter().find(|group| group.name == name) {
                return group.id;
            }
            let id = self.groups.iter().map(|group| group.id).max().unwrap_or(0) + 1;
            self.groups.push(super::groups::Group {
                id,
                name,
                editing: None,
            });
            id
        });
        for pane in &mut session.panes {
            pane.terminal
                .update(cx, |terminal, cx| terminal.move_to_window(window, cx));
            pane._events = Self::watch_terminal(&pane.terminal, window, cx);
        }
        let index = index.min(self.sessions.len());
        if !self.sessions.is_empty() && index <= self.active {
            self.active += 1;
        }
        let id = session.id;
        self.sessions.insert(index, session);
        self.arrange();
        let index = self
            .sessions
            .iter()
            .position(|session| session.id == id)
            .unwrap();
        self.select(index, window, cx);
        self.keep(cx);
    }
}
