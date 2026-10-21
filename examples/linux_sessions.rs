#[cfg(target_os = "linux")]
fn main() {
    use ely_gpui_component::Assets;
    use elyterminal::{
        app,
        settings::{SettingsStore, StartIn},
        term::shell::{ShellIntegration, launch},
        workspace::{CloseSession, NewSession, SelectSession},
    };
    use gpui::{Action, Focusable};
    use std::time::Duration;

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("elyterminal=info"))
        .init();
    gpui_platform::linux(gpui::WindowingModes::WAYLAND | gpui::WindowingModes::X11)
        .with_assets(Assets)
        .run(|cx| {
            ely_gpui_component::init(cx).unwrap();
            app::start(cx).unwrap();
            let launch = launch(
                &SettingsStore::get(cx).shell,
                std::env::temp_dir(),
                &cx.global::<ShellIntegration>().0,
            )
            .unwrap();
            let handle = app::open(launch, cx).unwrap();
            cx.spawn(async move |cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(300))
                    .await;
                for (action, count, focused) in [
                    (Box::new(NewSession) as Box<dyn Action>, 2, 1),
                    (Box::new(SelectSession(0)), 2, 0),
                    (Box::new(CloseSession), 1, 0),
                ] {
                    handle
                        .update(cx, |_, window, cx| window.dispatch_action(action, cx))
                        .unwrap();
                    cx.background_executor()
                        .timer(Duration::from_millis(100))
                        .await;
                    handle
                        .update(cx, |workspace, window, cx| {
                            assert_eq!(workspace.sessions().len(), count);
                            let session = &workspace.sessions()[focused];
                            assert!(session.terminal.focus_handle(cx).is_focused(window));
                        })
                        .unwrap();
                }
                cx.update(|cx| {
                    SettingsStore::update(cx, |settings| settings.start_in = StartIn::Session)
                })
                .unwrap();
                handle
                    .update(cx, |_, window, cx| {
                        window.dispatch_action(Box::new(NewSession), cx)
                    })
                    .unwrap();
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                handle
                    .update(cx, |workspace, _, _| {
                        assert_eq!(workspace.sessions().len(), 2);
                        assert_eq!(
                            workspace.sessions()[1].folder,
                            workspace.sessions()[0].folder
                        );
                    })
                    .unwrap();
                println!(
                    "SESSION ACTION REGRESSION PASSED: new, select, close, focus, Start in Session"
                );
                cx.update(|cx| cx.quit());
            })
            .detach();
        });
}

#[cfg(not(target_os = "linux"))]
fn main() {}
