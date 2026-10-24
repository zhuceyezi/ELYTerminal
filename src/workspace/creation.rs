use super::*;
use crate::term::{config, spawn};

impl Workspace {
    pub(crate) fn empty(
        folder: std::path::PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let git = cx.new(|cx| GitPanel::new(folder.clone(), window, cx));
        // A change the Git panel made shows in Files' marks too.
        let acted = cx.subscribe(&git, |this, _, _: &Acted, cx| {
            if let Some(session) = this.sessions.get(this.active) {
                let folder = session.folder.clone();
                this.files.update(cx, |files, cx| files.show(&folder, cx));
            }
        });
        Self {
            focus: cx.focus_handle(),
            sessions: Vec::new(),
            active: 0,
            files: cx.new(|_| Files::new(folder.clone())),
            git,
            background: Blur::new(window, SettingsStore::get(cx)),
            dragged: Dragged::default(),
            ticking: None,
            _settings: cx.observe_global_in::<SettingsStore>(window, |this, window, cx| {
                this.background.follow(window, SettingsStore::get(cx));
                cx.notify();
            }),
            _acted: acted,
            session_drag: None,
        }
    }

    pub(crate) fn start_session(
        &mut self,
        launch: Launch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let (pty, events) = spawn(&launch, (80, 24), (8, 16), config(SettingsStore::get(cx)))?;
        self.add(pty, events, launch, window, cx);
        self.select(self.sessions.len() - 1, window, cx);
        Ok(())
    }
}
