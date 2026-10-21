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
