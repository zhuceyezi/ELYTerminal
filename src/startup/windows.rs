use gpui::{App, Global, WindowHandle};

use super::{Request, activation};
use crate::{app, workspace::Workspace};

#[derive(Default)]
struct Recent(Vec<WindowHandle<Workspace>>);
impl Global for Recent {}

pub(crate) fn track(handle: WindowHandle<Workspace>, cx: &mut App) -> anyhow::Result<()> {
    if !cx.has_global::<Recent>() {
        cx.set_global(Recent::default());
    }
    remember(handle, cx);
    handle.update(cx, |_, window, cx| {
        cx.observe_window_activation(window, move |_, window, cx| {
            if window.is_window_active() {
                remember(handle, cx);
            }
        })
        .detach();
    })?;
    Ok(())
}

fn remember(handle: WindowHandle<Workspace>, cx: &mut App) {
    let existing = cx.windows();
    let recent = cx.global_mut::<Recent>();
    recent
        .0
        .retain(|other| *other != handle && existing.contains(&(*other).into()));
    recent.0.push(handle);
}

pub(super) fn open(request: Request, cx: &mut App) -> anyhow::Result<()> {
    let launch = request.launch(cx)?;
    let existing = cx.windows();
    let target = (!request.new_window)
        .then(|| {
            cx.try_global::<Recent>().and_then(|recent| {
                recent
                    .0
                    .iter()
                    .rev()
                    .find(|handle| existing.contains(&(**handle).into()))
                    .copied()
            })
        })
        .flatten();
    let window = if let Some(window) = target {
        window.update(cx, |this, window, cx| {
            this.start_session(launch, window, cx)
        })??;
        log::info!("startup: reused terminal window");
        window
    } else {
        let window = app::open(launch, cx)?;
        log::info!("startup: created terminal window");
        window
    };
    window.update(cx, |_, window, cx| {
        cx.activate(true);
        if let Err(error) = activation::activate(window, request.activation_token.as_deref()) {
            log::warn!("startup: activation failed: {error:#}");
            window.activate_window();
        }
    })?;
    Ok(())
}
