#[cfg(target_os = "linux")]
fn main() {
    use ely_gpui_component::Assets;
    use elyterminal::{
        app,
        sessions::{Program, saved},
        settings::SettingsStore,
        term::shell::{ShellIntegration, Start, launch},
        workspace::{Keeps, SplitRight},
    };
    use std::time::Duration;

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("elyterminal=info"))
        .init();
    assert!(
        std::env::var_os("ELY_RESTORE_TEST_ISOLATED").is_some(),
        "run only with isolated HOME/XDG_CONFIG_HOME"
    );
    gpui_platform::linux(gpui::WindowingModes::X11).with_assets(Assets).run(|cx| {
        ely_gpui_component::init(cx).unwrap();
        app::start(cx).unwrap();
        cx.set_global(Keeps(true));
        let shell = SettingsStore::get(cx).shell.clone();
        let program = Program { shell: shell.clone(), args: vec![], written: false };
        let started = launch(&shell, &[], Start::In(std::env::temp_dir()), &cx.global::<ShellIntegration>().0).unwrap();
        let first = app::open(started, program.clone(), cx).unwrap();
        let started = launch(&shell, &[], Start::In(std::env::temp_dir()), &cx.global::<ShellIntegration>().0).unwrap();
        let second = app::open(started, program, cx).unwrap();
        cx.spawn(async move |cx| {
            cx.background_executor().timer(Duration::from_millis(300)).await;
            for window in [first, second] {
                window.update(cx, |workspace, window, cx| {
                    let id = workspace.sessions()[0].id;
                    workspace.new_group(id, window, cx);
                    workspace.end_group_rename(1, false, window, cx);
                }).unwrap();
            }
            first.update(cx, |_, window, cx| window.dispatch_action(Box::new(SplitRight), cx)).unwrap();
            cx.background_executor().timer(Duration::from_millis(1000)).await;
            let left = saved::load(&saved::path().unwrap()).unwrap().unwrap();
            assert_eq!(left.sessions.len(), 2, "both windows saved");
            assert_eq!(left.groups.len(), 2, "same local group ID kept separate");
            assert_ne!(left.sessions[0].group, left.sessions[1].group);
            assert_eq!(left.sessions[0].layout.panes().len(), 2);
            let id = first.update(cx, |workspace, _, _| workspace.sessions()[0].id).unwrap();
            first.update(cx, |workspace, window, cx| workspace.transfer_session(id, second, 0, window, cx)).unwrap().unwrap();
            cx.background_executor().timer(Duration::from_millis(1000)).await;
            let moved = saved::load(&saved::path().unwrap()).unwrap().unwrap();
            assert_eq!(moved.sessions.len(), 2, "transfer neither loses nor duplicates a session");
            assert_eq!(moved.sessions.iter().map(|session| session.layout.panes().len()).sum::<usize>(), 3);
            cx.update(|cx| cx.set_global(Keeps(false)));
            let (shell, args, folder) = moved.sessions[0].layout.panes()[0];
            let restored = cx.update(|cx| {
                let started = launch(shell, args, Start::In(folder.into()), &cx.global::<ShellIntegration>().0).unwrap();
                app::open(started, Program { shell: shell.into(), args: args.into(), written: false }, cx).unwrap()
            });
            let expected = moved.clone();
            restored.update(cx, |workspace, window, cx| workspace.restore(moved, window, cx)).unwrap();
            cx.background_executor().timer(Duration::from_millis(300)).await;
            restored.update(cx, |workspace, _, _| {
                assert_eq!(workspace.sessions().len(), expected.sessions.len());
                for (session, saved) in workspace.sessions().iter().zip(&expected.sessions) {
                    assert_eq!(session.panes.len(), saved.layout.panes().len());
                    assert_eq!(session.name, saved.name);
                    assert_eq!(session.group, saved.group);
                    assert_eq!(session.focused, saved.focused);
                }
            }).unwrap();
            println!("RESTORE REGRESSION PASSED: multi-window save, colliding group IDs, split panes, transfer without duplication, restore");
            cx.update(|cx| cx.quit());
        }).detach();
    });
}

#[cfg(not(target_os = "linux"))]
fn main() {}
