mod activation;
mod ipc;
mod request;
mod windows;

use anyhow::Context as _;
use ely_gpui_component::Assets;
use futures::StreamExt;
use gpui::App;

use crate::app;
use ipc::Instance;
pub use request::Request;
pub(crate) use windows::track;

/// Starts a graphical owner, or sends this invocation to the existing owner.
pub fn run() -> anyhow::Result<()> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments.len() == 1 && arguments[0] == "--help" {
        println!(
            "ELYTerminal\n\nUsage: elyterminal [--new-window] [--working-directory DIR] [-e COMMAND [ARGS...]]\n\nLinux reuses the most recently active terminal window by default.\nDirectory aliases: --workdir, --directory. A single command string runs through /bin/sh -c."
        );
        return Ok(());
    }
    if arguments.len() == 1 && arguments[0] == "--version" {
        println!(
            "ELYTerminal {} (Linux window reuse)",
            env!("CARGO_PKG_VERSION")
        );
        return Ok(());
    }
    let request = Request::parse(arguments, std::env::current_dir()?)?;
    let Instance::Owner(owner) = ipc::claim(&request)? else {
        return Ok(());
    };
    let mut requests = owner.listen()?;
    gpui_platform::linux(gpui::WindowingModes::WAYLAND | gpui::WindowingModes::X11)
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            let started = (|| {
                ely_gpui_component::init(cx).context("startup: Ely initialization failed")?;
                app::start(cx)?;
                windows::open(request, cx)?;
                anyhow::Ok(())
            })();
            if let Err(error) = started {
                eprintln!("{error:#}");
                log::error!("{error:#}");
                std::process::exit(1);
            }
            cx.spawn(async move |cx| {
                while let Some(incoming) = requests.next().await {
                    let result = cx
                        .update(|cx| windows::open(incoming.request, cx))
                        .map_err(|error| format!("startup: {error:#}"));
                    if let Err(error) = &result {
                        log::error!("{error}");
                    }
                    let _ = incoming.reply.send(result);
                }
            })
            .detach();
        });
    drop(owner);
    Ok(())
}
