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
        let files = cx.new(|cx| {
            let mut files = Files::new(folder.clone());
            files.poll(cx);
            files
        });
        let here = cx.subscribe_in(&files, window, |this, _, Here(folder), window, cx| {
            if let Err(error) = this.here(folder.clone(), window, cx) {
                folder::failed(error, window, cx);
            }
        });
        Self {
            focus: cx.focus_handle(),
            sessions: Vec::new(),
            active: 0,
            files,
            git,
            background: Blur::new(window, SettingsStore::get(cx)),
            dragged: Dragged::default(),
            ticking: None,
            found: crate::profiles::detected(),
            choosing_folder: false,
            folder_actions: false,
            _settings: cx.observe_global_in::<SettingsStore>(window, |this, window, cx| {
                this.background.follow(window, SettingsStore::get(cx));
                let settings = SettingsStore::get(cx);
                if !settings.files_menu || !settings.show_panel || settings.panel != PanelTab::Files
                {
                    this.folder_actions = false;
                }
                cx.notify();
            }),
            _acted: acted,
            _here: here,
            session_layout: transfer::Layout::default(),
        }
    }

    pub(crate) fn start_session(
        &mut self,
        launch: Launch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let (pty, events) = spawn(&launch, (80, 24), (8, 16), config(SettingsStore::get(cx)))?;
        self.add(pty, events, launch, Vec::new(), window, cx);
        self.select(self.sessions.len() - 1, window, cx);
        Ok(())
    }
}
