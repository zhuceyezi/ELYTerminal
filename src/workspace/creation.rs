use super::*;
use crate::{
    settings::StartIn,
    term::{
        config,
        shell::{ShellIntegration, launch},
        spawn,
    },
};

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

    /// A session of the shell the settings name, in home or the shown session's folder.
    pub(super) fn new_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(home) = dirs::home_dir() else {
            log::error!("app: no home folder, so no new session");
            return;
        };
        let settings = SettingsStore::get(cx);
        let folder = match settings.start_in {
            StartIn::Home => home,
            StartIn::Session => self
                .sessions
                .get(self.active)
                .map_or(home, |session| session.folder.clone()),
        };
        let started = launch(&settings.shell, folder, &cx.global::<ShellIntegration>().0)
            .and_then(|launch| self.start_session(launch, window, cx));
        if let Err(error) = started {
            log::error!("app: no new session: {error:#}");
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
