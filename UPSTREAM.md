# Upstream history and synchronization

This fork adds Linux window reuse and live session detachment on top of
upstream v0.1.8, synchronized through commit `e477637` on 2026-10-24.
It preserves the original Git history.
It is a GitHub fork of the project's GitHub repository, whose v0.1.7 tag is
identical to that on the original Gitea server.

## Remotes

- `origin`: `https://github.com/zhuceyezi/ELYTerminal.git` (this fork)
- `upstream`: `https://git.zacharyzhang.com/ZacharyZhang-NY/ELYTerminal.git`
- `github-upstream`: `https://github.com/ELY-Open-Source/ELYTerminal.git`

After cloning this fork, configure the original server once:

```sh
git remote add upstream https://git.zacharyzhang.com/ZacharyZhang-NY/ELYTerminal.git
git remote add github-upstream https://github.com/ELY-Open-Source/ELYTerminal.git
```

The customization lives on `linux-window-reuse`. To merge future changes from
the original server while retaining this fork's commits:

```sh
git switch linux-window-reuse
git fetch github-upstream --tags
git merge github-upstream/main
scripts/check.sh
cargo build --release --locked
git push origin linux-window-reuse
```

If Git reports conflicts, resolve them and rerun the checks before pushing.
Use `git merge --abort` to abandon an unfinished merge. Never solve an ordinary
upstream update with `--allow-unrelated-histories` or a force push.

The GitHub mirror can be merged in the same way with `github-upstream/main`.
GitHub's **Sync fork** button synchronizes only its GitHub parent, not the Gitea
remote. This fork cannot create cross-site pull requests through that button.

To offer the customization back to the original server, its maintainer can
fetch this fork's `linux-window-reuse` branch and merge it, or apply patches
created with `git format-patch v0.1.7..linux-window-reuse`. No history conversion
is needed. A pull request on the Gitea site requires a repository and account
on that site; publication here does not create one automatically.

## Local build

The pinned toolchain is Rust 1.95.0. Run `scripts/check.sh` before a release
build. X11 tests need Xvfb, and shell integration tests need fish and zsh.
`cargo run --example linux_sessions` additionally exercises the real Linux
window's session actions. Run that example with a temporary `XDG_CONFIG_HOME`
to keep its settings changes separate from personal settings.

## v0.1.8 integration

Upstream row reordering and session duplication coexist with live detachment.
Dragging within the list reorders sessions; crossing the window edge transfers
the existing terminal and PTY. Detachment and Escape clear the active reorder
drag so its preview cannot remain attached to the old window. Duplication
retains the source shell and directory, including after a session is detached.
Linux single-instance IPC, external working directories and activation tokens
remain in this branch. Local package versions are read from Cargo.toml.

Validation on KDE Wayland: all 287 tests and repository checks pass; native
release build, session duplication/reordering/focus, live row reorder followed
by detach, same process and terminal identity after transfer, directory/title
updates, Escape, and last-session transfer pass. Single-instance checks cover
six concurrent launches, literal Chinese/quoted directories, explicit windows,
invalid launch rejection and reuse of the most recently activated window.
