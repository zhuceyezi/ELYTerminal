//! Restores only the first plain launch; later IPC requests open exactly what they asked for.

use gpui::App;

use crate::{
    app,
    sessions::{Program, saved},
    settings::SettingsStore,
    term::shell::{ShellIntegration, Start, launch},
    workspace::Keeps,
};

pub(super) fn open(keeps: bool, cx: &mut App) -> anyhow::Result<bool> {
    cx.set_global(Keeps(keeps));
    if !keeps {
        return Ok(false);
    }
    let saved = match saved::path().and_then(|path| saved::load(&path)) {
        Ok(saved) => saved,
        Err(error) => {
            log::error!("{error:#}; this run keeps no sessions");
            cx.set_global(Keeps(false));
            return Ok(false);
        }
    };
    if !SettingsStore::get(cx).restore_sessions {
        return Ok(false);
    }
    let Some(saved) = saved.and_then(|saved| saved.kept(|folder| folder.is_dir())) else {
        return Ok(false);
    };
    let (shell, args, folder) = saved.sessions[0].layout.panes()[0];
    let program = Program {
        shell: shell.into(),
        args: args.into(),
        written: false,
    };
    let started = match launch(
        shell,
        args,
        Start::In(folder.into()),
        &cx.global::<ShellIntegration>().0,
    ) {
        Ok(started) => started,
        Err(error) => {
            log::error!("sessions: the first saved pane does not start: {error:#}");
            cx.set_global(Keeps(false));
            return Ok(false);
        }
    };
    let window = app::open(started, program, cx)?;
    window.update(cx, |workspace, window, cx| {
        workspace.restore(saved, window, cx);
        window.activate_window();
    })?;
    cx.activate(true);
    Ok(true)
}
