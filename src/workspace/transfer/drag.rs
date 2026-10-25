use super::{Workspace, list::Layout};
use ely_gpui_component::theme::ActiveTheme;
use gpui::{
    App, AppContext, DispatchPhase, Entity, EntityId, Global, IntoElement, MouseButton,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, Styled, Window, WindowHandle, canvas, px,
};
use std::time::Duration;

#[derive(Clone)]
pub(super) struct Held {
    pub source: WindowHandle<Workspace>,
    pub id: EntityId,
    pub title: String,
    pub height: Pixels,
    pub target: Option<(WindowHandle<Workspace>, usize)>,
    native: bool,
    outside: bool,
}

#[derive(Default)]
pub(super) struct Drag {
    pub held: Option<Held>,
    armed: bool,
    blocked: bool,
    #[cfg(target_os = "linux")]
    native: Option<super::native::Native>,
}
impl Global for Drag {}

pub(super) fn init(window: &Window, cx: &mut App) {
    if cx.has_global::<Drag>() {
        return;
    }
    let drag = Drag::default();
    #[cfg(target_os = "linux")]
    let drag = Drag {
        native: match super::native::Native::new(window) {
            Ok(native) => native,
            Err(error) => {
                log::error!("session drag: native transport unavailable: {error:#}");
                None
            }
        },
        ..drag
    };
    cx.set_global(drag);
    #[cfg(target_os = "linux")]
    cx.spawn(async |cx| {
        loop {
            cx.background_executor()
                .timer(Duration::from_millis(8))
                .await;
            cx.update(poll);
        }
    })
    .detach();
}

pub(super) fn begin(
    workspace: &Entity<Workspace>,
    id: EntityId,
    offset: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<Label> {
    if cx.global::<Drag>().blocked {
        let handle = window.window_handle();
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| cx.stop_active_drag(window));
        });
        return cx.new(|_| Label(String::new()));
    }
    let source = window
        .window_handle()
        .downcast::<Workspace>()
        .expect("session window");
    let session = workspace
        .read(cx)
        .sessions
        .iter()
        .find(|s| s.terminal.entity_id() == id)
        .expect("dragged session");
    let held = Held {
        source,
        id,
        title: session.named().to_owned(),
        height: workspace
            .read(cx)
            .session_layout
            .rows
            .get(&id)
            .map_or(px(44.), |bounds| bounds.size.height),
        target: None,
        native: false,
        outside: false,
    };
    let colors = [
        cx.theme().colors.overlay,
        cx.theme().colors.fg,
        cx.theme().colors.border_strong,
    ];
    #[cfg(target_os = "linux")]
    let native = cx
        .global_mut::<Drag>()
        .native
        .as_mut()
        .is_some_and(
            |native| match native.start(window, &held.title, offset, colors) {
                Ok(()) => true,
                Err(error) => {
                    log::error!("session drag: cannot lift label: {error:#}");
                    false
                }
            },
        );
    #[cfg(not(target_os = "linux"))]
    let native = false;
    cx.global_mut::<Drag>().held = Some(Held { native, ..held });
    let layout = workspace.read(cx).session_layout.clone();
    hover(source, &layout, window.mouse_position(), cx);
    if native {
        let handle = window.window_handle();
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| cx.stop_active_drag(window));
        });
    }
    log::info!("session drag: preview started for {id:?}");
    cx.refresh_windows();
    let title = cx.global::<Drag>().held.as_ref().unwrap().title.clone();
    cx.new(|_| Label(title))
}

pub(in crate::workspace) fn cancel(cx: &mut App) -> bool {
    if !cx.has_global::<Drag>()
        || (cx.global::<Drag>().held.is_none() && !cx.global::<Drag>().armed)
    {
        return false;
    }
    cx.global_mut::<Drag>().blocked = true;
    cx.global_mut::<Drag>().armed = false;
    #[cfg(target_os = "linux")]
    if let Some(native) = cx.global_mut::<Drag>().native.as_mut() {
        native.finish();
    }
    cx.global_mut::<Drag>().held = None;
    log::info!("session drag: cancelled; original sessions and order kept");
    cx.refresh_windows();
    true
}

pub(super) fn hover(
    target: WindowHandle<Workspace>,
    layout: &Layout,
    position: Point<Pixels>,
    cx: &mut App,
) {
    let Some(held) = cx.global::<Drag>().held.clone() else {
        return;
    };
    let index = layout.landing(position, (target == held.source).then_some(held.id));
    let next = index.map(|index| (target, index));
    if let Some(current) = &mut cx.global_mut::<Drag>().held {
        current.outside = target != held.source || !layout.window.contains(&position);
        if current.target != next {
            log::debug!(
                "session drag: preview slot {:?} at {position:?}; rows {:?}",
                next.map(|(_, index)| index),
                layout.rows
            );
            current.target = next;
            cx.refresh_windows();
        }
    }
    #[cfg(target_os = "linux")]
    if held.native
        && let Some(native) = cx.global_mut::<Drag>().native.as_mut()
    {
        native.accept(next.is_some());
    }
}

fn leave(cx: &mut App) {
    if let Some(held) = &mut cx.global_mut::<Drag>().held {
        held.target = None;
        held.outside = true;
        cx.refresh_windows();
    }
}

pub(super) fn release(cx: &mut App) {
    cx.global_mut::<Drag>().armed = false;
    let Some(held) = cx.global_mut::<Drag>().held.take() else {
        return;
    };
    #[cfg(target_os = "linux")]
    if let Some(native) = cx.global_mut::<Drag>().native.as_mut() {
        native.finish();
    }
    cx.refresh_windows();
    cx.defer(move |cx| {
        let result = held.source.update(cx, |source, window, cx| {
            if let Some((target, index)) = held.target {
                if target == held.source {
                    if let Some(from) = source
                        .sessions
                        .iter()
                        .position(|s| s.terminal.entity_id() == held.id)
                    {
                        source.reorder(from, index.min(source.sessions.len() - 1), cx);
                    }
                    Ok(())
                } else {
                    source.transfer_session(held.id, target, index, window, cx)
                }
            } else if held.outside {
                let origin = window.window_bounds().get_bounds().origin;
                source
                    .detach_session(held.id, origin, window, cx)
                    .map(|_| ())
            } else {
                Ok(())
            }
        });
        match result {
            Ok(Ok(())) => log::info!("session drag: drop committed"),
            Ok(Err(error)) => log::error!("session drag: session stayed in source: {error:#}"),
            Err(error) => log::warn!("session drag: source closed before drop: {error:#}"),
        }
    });
}

#[cfg(target_os = "linux")]
fn poll(cx: &mut App) {
    use super::native::{Event, Native};
    let events = match cx.global_mut::<Drag>().native.as_mut().map(Native::poll) {
        Some(Ok(events)) => events,
        Some(Err(error)) => {
            log::error!("session drag: {error:#}");
            cancel(cx);
            return;
        }
        None => return,
    };
    for event in events {
        if cx.global::<Drag>().held.is_none() {
            break;
        }
        match event {
            Event::Hover(surface, position) => {
                let windows: Vec<_> = cx
                    .windows()
                    .into_iter()
                    .filter_map(|w| w.downcast::<Workspace>())
                    .collect();
                let mut found = false;
                for handle in windows {
                    let matched = handle
                        .update(cx, |workspace, window, cx| {
                            if Native::surface(window).ok().as_ref() != Some(&surface) {
                                return false;
                            }
                            hover(handle, &workspace.session_layout, position, cx);
                            true
                        })
                        .unwrap_or(false);
                    if matched {
                        found = true;
                        break;
                    }
                }
                if !found {
                    leave(cx);
                }
            }
            Event::Leave => leave(cx),
            Event::Drop => release(cx),
            Event::End { cancelled: true } | Event::Cancel => {
                cancel(cx);
            }
            Event::End { cancelled: false } => release(cx),
        }
    }
}

pub(in crate::workspace) fn handlers(workspace: &Entity<Workspace>) -> impl IntoElement {
    let workspace = workspace.clone();
    canvas(
        |_, _, _| (),
        move |_, (), window, _| {
            let moving = workspace.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture
                    || cx
                        .try_global::<Drag>()
                        .and_then(|d| d.held.as_ref())
                        .is_none_or(|h| h.native)
                {
                    return;
                }
                let layout = moving.read(cx).session_layout.clone();
                let handle = window.window_handle().downcast::<Workspace>().unwrap();
                hover(handle, &layout, event.position, cx);
            });
            window.on_mouse_event(move |event: &gpui::MouseDownEvent, phase, window, cx| {
                if phase == DispatchPhase::Capture
                    && event.button == MouseButton::Right
                    && cancel(cx)
                {
                    cx.stop_active_drag(window);
                    cx.stop_propagation();
                }
            });
            window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                if phase == DispatchPhase::Capture
                    && event.button == MouseButton::Left
                    && cx
                        .try_global::<Drag>()
                        .and_then(|d| d.held.as_ref())
                        .is_some_and(|h| !h.native)
                {
                    release(cx);
                    cx.stop_active_drag(window);
                    cx.stop_propagation();
                }
            });
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

pub(super) struct Label(String);
impl gpui::Render for Label {
    fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        use gpui::{ParentElement, Styled};
        gpui::div()
            .px_3()
            .py_2()
            .rounded_md()
            .border_1()
            .bg(cx.theme().colors.overlay)
            .border_color(cx.theme().colors.border_strong)
            .text_color(cx.theme().colors.fg)
            .child(self.0.clone())
    }
}

pub(super) fn arm(cx: &mut App) {
    let drag = cx.global_mut::<Drag>();
    drag.armed = true;
    drag.blocked = false;
}
