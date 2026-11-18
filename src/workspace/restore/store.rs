//! One writer for all terminal windows, with group identifiers local to each snapshot.

use std::{collections::HashMap, time::Duration};

use gpui::{App, Context, EntityId, Global, Task, WeakEntity};

use super::{Saved, Workspace, saved};
use crate::settings::save;

#[derive(Default)]
struct Store {
    workspaces: Vec<WeakEntity<Workspace>>,
    saving: Option<Task<()>>,
    last: Option<Saved>,
}

impl Global for Store {}

pub(super) fn register(cx: &mut Context<Workspace>) {
    if !cx.has_global::<Store>() {
        cx.set_global(Store::default());
    }
    let weak = cx.entity().downgrade();
    cx.global_mut::<Store>().workspaces.push(weak);
}

fn merge(parts: Vec<Saved>) -> Saved {
    let mut all = Saved {
        sessions: Vec::new(),
        groups: Vec::new(),
        shown: 0,
    };
    for mut part in parts {
        let mut ids = HashMap::new();
        for mut group in part.groups {
            let id = all.groups.len() as u64 + 1;
            ids.insert(group.id, id);
            group.id = id;
            all.groups.push(group);
        }
        for session in &mut part.sessions {
            session.group = session.group.and_then(|id| ids.get(&id).copied());
        }
        if all.sessions.is_empty() {
            all.shown = part.shown;
        }
        all.sessions.extend(part.sessions);
    }
    all
}

fn snapshot(own: Option<(EntityId, Saved)>, cx: &App) -> Saved {
    let mut parts = Vec::new();
    for weak in &cx.global::<Store>().workspaces {
        if let Some(entity) = weak.upgrade() {
            if let Some((id, saved)) = &own
                && entity.entity_id() == *id
            {
                parts.push(saved.clone());
            } else {
                parts.push(entity.read(cx).snapshot(cx));
            }
        }
    }
    merge(parts)
}

fn write(now: Saved, cx: &mut App) {
    match saved::path().and_then(|path| save(&path, &now, "sessions")) {
        Ok(()) => cx.global_mut::<Store>().last = Some(now),
        Err(error) => log::error!("{error:#}"),
    }
}

pub(super) fn flush(own: Saved, cx: &mut Context<Workspace>) {
    cx.global_mut::<Store>().saving = None;
    let now = snapshot(Some((cx.entity_id(), own)), cx);
    write(now, cx);
}

pub(super) fn keep(cx: &mut Context<Workspace>) {
    if cx.global::<Store>().saving.is_some() {
        return;
    }
    let task = cx.spawn(async move |_, cx| {
        cx.background_executor()
            .timer(Duration::from_millis(500))
            .await;
        cx.update(|cx| {
            if !Workspace::keeps(cx) {
                cx.global_mut::<Store>().saving = None;
                return;
            }
            let now = snapshot(None, cx);
            if cx.global::<Store>().last.as_ref() != Some(&now) {
                write(now, cx);
            }
            cx.global_mut::<Store>().saving = None;
        });
    });
    cx.global_mut::<Store>().saving = Some(task);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::saved::{SavedGroup, SavedLayout, SavedSession};

    #[test]
    fn windows_with_the_same_group_id_keep_distinct_names_and_all_panes() {
        let window = |name: &str| Saved {
            sessions: vec![SavedSession {
                name: name.into(),
                group: Some(1),
                focused: 0,
                layout: SavedLayout::Pane {
                    shell: "sh".into(),
                    args: vec![],
                    folder: "/".into(),
                },
            }],
            groups: vec![SavedGroup {
                id: 1,
                name: name.into(),
            }],
            shown: 0,
        };
        let all = merge(vec![window("first"), window("second")]);
        assert_eq!(all.sessions.len(), 2);
        assert_eq!(all.groups[0].name, "first");
        assert_eq!(all.groups[1].name, "second");
        assert_ne!(all.sessions[0].group, all.sessions[1].group);
        assert_eq!(merge(vec![]).sessions.len(), 0);
        let mut first = window("same name");
        let second = window("same name");
        first.sessions[0].focused = 1;
        first.sessions[0].layout = SavedLayout::Split {
            down: true,
            share: 0.3,
            first: Box::new(first.sessions[0].layout.clone()),
            second: Box::new(second.sessions[0].layout.clone()),
        };
        let layout = first.sessions[0].layout.clone();
        let mut shown = first.clone();
        shown.sessions.push(shown.sessions[0].clone());
        shown.shown = 1;
        let empty = Saved {
            sessions: vec![],
            groups: vec![],
            shown: 0,
        };
        assert_eq!(merge(vec![empty, shown, second.clone()]).shown, 1);
        let all = merge(vec![first, second]);
        assert_eq!(
            all.groups.len(),
            2,
            "identical labels do not collapse distinct groups"
        );
        assert_eq!(all.sessions[0].layout, layout);
        assert_eq!(all.sessions[0].focused, 1);
        assert_eq!(all.sessions[0].layout.panes().len(), 2);
    }
}
