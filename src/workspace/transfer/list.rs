use ely_gpui_component::{motion::Flip, theme::ActiveTheme};
use gpui::{
    AnyElement, Bounds, Context, EntityId, InteractiveElement, IntoElement, ParentElement, Pixels,
    Point, StatefulInteractiveElement, Styled, Window, canvas, div, prelude::FluentBuilder,
};
use std::{collections::HashMap, time::Instant};

use super::{
    Workspace,
    drag::{self, Drag},
};
use crate::{sessions, settings::SettingsStore};

#[derive(Clone, Default)]
pub(in crate::workspace) struct Layout {
    pub window: Bounds<Pixels>,
    bounds: Bounds<Pixels>,
    pub rows: HashMap<EntityId, Bounds<Pixels>>,
    gap: Option<(usize, Bounds<Pixels>)>,
}

impl Layout {
    pub fn landing(&self, position: Point<Pixels>, excluded: Option<EntityId>) -> Option<usize> {
        if !self.bounds.contains(&position) {
            return None;
        }
        if let Some((index, bounds)) = self.gap
            && bounds.contains(&position)
        {
            return Some(index);
        }
        let mut rows: Vec<_> = self
            .rows
            .iter()
            .filter(|(id, _)| Some(**id) != excluded)
            .map(|(_, bounds)| bounds)
            .collect();
        rows.sort_by(|a, b| a.top().partial_cmp(&b.top()).unwrap());
        Some(
            rows.iter()
                .take_while(|row| row.top() + row.size.height / 2. < position.y)
                .count(),
        )
    }
}

pub(in crate::workspace) fn list(
    this: &Workspace,
    now: Instant,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) -> AnyElement {
    if !SettingsStore::get(cx).drag_sessions {
        let workspace = cx.entity();
        let home = dirs::home_dir();
        let reset = workspace.clone();
        cx.defer(move |cx| reset.update(cx, |this, _| this.session_layout = Layout::default()));
        return ely_gpui_component::lists::List::new()
            .children(this.sessions.iter().enumerate().map(|(index, session)| {
                sessions::row(
                    &workspace,
                    index,
                    session,
                    index == this.active,
                    home.as_deref(),
                    now,
                    cx,
                )
            }))
            .into_any_element();
    }
    drag::init(window, cx);
    let workspace = cx.entity();
    let handle = window.window_handle().downcast::<Workspace>().unwrap();
    let held = cx.global::<Drag>().held.clone();
    let omitted = held
        .as_ref()
        .filter(|held| held.source == handle)
        .map(|held| held.id);
    let insertion = held.as_ref().and_then(|held| {
        held.target
            .filter(|(target, _)| *target == handle)
            .map(|(_, index)| index)
    });
    let home = dirs::home_dir();
    let rows: Vec<_> = this
        .sessions
        .iter()
        .enumerate()
        .filter(|(_, session)| Some(session.terminal.entity_id()) != omitted)
        .collect();
    let live: Vec<_> = rows
        .iter()
        .map(|(_, session)| session.terminal.entity_id())
        .collect();
    let mut flip = Flip::new("session-rows");
    let colors = cx.theme().colors.clone();
    for index in 0..=rows.len() {
        if insertion == Some(index) {
            let held = held.as_ref().unwrap();
            let gap = div()
                .relative()
                .h(held.height)
                .w_full()
                .border_1()
                .rounded_md()
                .border_color(colors.focus.alpha(0.55))
                .bg(colors.focus.alpha(0.08))
                .px_3()
                .flex()
                .items_center()
                .text_color(colors.fg_muted)
                .child(held.title.clone())
                .child(
                    canvas(
                        {
                            let workspace = workspace.clone();
                            move |bounds, _, cx| {
                                workspace.update(cx, |this, _| {
                                    this.session_layout.gap = Some((index, bounds))
                                });
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
                );
            flip = flip.row("session-drop-gap", gap);
        }
        let Some((source_index, session)) = rows.get(index).copied() else {
            continue;
        };
        let id = session.terminal.entity_id();
        let pid = session.terminal.read(cx).pid();
        let row = sessions::row(
            &workspace,
            source_index,
            session,
            source_index == this.active,
            home.as_deref(),
            now,
            cx,
        );
        let source = workspace.clone();
        let measured = workspace.clone();
        let row = div()
            .id(("session-label", pid as usize))
            .relative()
            .pb_0p5()
            .child(row)
            .child(
                canvas(
                    move |bounds, _, cx| {
                        measured.update(cx, |this, _| {
                            this.session_layout.rows.insert(id, bounds);
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| drag::arm(cx))
            .when(session.editing.is_none(), |row| {
                row.on_drag(id, move |_, offset, window, cx| {
                    drag::begin(&source, id, offset, window, cx)
                })
            });
        flip = flip.row(pid.to_string(), row);
    }
    div()
        .relative()
        .size_full()
        .child(flip)
        .child(
            canvas(
                move |bounds, window, cx| {
                    workspace.update(cx, |this, _| {
                        this.session_layout.bounds = bounds;
                        this.session_layout.window =
                            Bounds::new(Point::default(), window.viewport_size());
                        this.session_layout.rows.retain(|id, _| live.contains(id));
                        if insertion.is_none() {
                            this.session_layout.gap = None;
                        }
                    });
                },
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
        .into_any_element()
}
