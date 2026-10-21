// A release build on Windows opens no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(target_os = "linux"))]
use ely_gpui_component::Assets;
#[cfg(not(target_os = "linux"))]
use elyterminal::{
    app,
    settings::SettingsStore,
    term::shell::{ShellIntegration, launch},
};
#[cfg(not(target_os = "linux"))]
use gpui::App;

#[cfg(not(target_os = "linux"))]
fn application() -> gpui::Application {
    gpui_platform::application()
}

#[cfg(not(target_os = "linux"))]
fn run(cx: &mut App) -> anyhow::Result<()> {
    app::start(cx)?;
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("app: no home folder"))?;
    let launch = launch(
        &SettingsStore::get(cx).shell,
        home,
        &cx.global::<ShellIntegration>().0,
    )?;
    app::open(launch, cx)?;
    cx.activate(true);
    Ok(())
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("elyterminal=info"))
        .init();
    #[cfg(target_os = "linux")]
    if let Err(error) = elyterminal::startup::run() {
        eprintln!("{error:#}");
        log::error!("{error:#}");
        std::process::exit(1);
    }
    #[cfg(not(target_os = "linux"))]
    application().with_assets(Assets).run(|cx: &mut App| {
        ely_gpui_component::init(cx).expect("Ely failed to start");
        if let Err(error) = run(cx) {
            log::error!("{error:#}");
            std::process::exit(1);
        }
    });
}
