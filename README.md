# ELYTerminal

Linux window-reuse fork on the `linux-window-reuse` branch, based on upstream v0.2.0. The `main` branch remains available for upstream synchronization. See [UPSTREAM.md](UPSTREAM.md) for preserving history and merging updates from the original Gitea server.

[![Release](https://img.shields.io/github/v/release/ELY-Open-Source/ELYTerminal?style=flat-square&color=24292f)](https://github.com/ELY-Open-Source/ELYTerminal/releases/latest)
[![License](https://img.shields.io/badge/license-MIT-3da639?style=flat-square)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.95-dea584?style=flat-square)](https://www.rust-lang.org)
[![GPUI](https://img.shields.io/badge/GPUI-0.3-5b4cff?style=flat-square)](https://crates.io/crates/gpui-pre)
[![Ely](https://img.shields.io/badge/Ely-components-6d28d9?style=flat-square)](https://github.com/ELY-Open-Source/Ely-GPUI-Components)
[![alacritty](https://img.shields.io/badge/alacritty-0.26-f46d01?style=flat-square)](https://github.com/alacritty/alacritty)

English / [中文](README_zh.md)

A terminal for macOS, Windows and Linux, in Rust on GPUI and Ely components. Sessions sit on the left, the terminal in the middle, Git or Files on the right. Every setting has a control in the settings window.

## Install

A [release on GitHub](https://github.com/ELY-Open-Source/ELYTerminal/releases) carries builds for each platform.

- macOS 11 or later, Apple silicon: open `ELYTerminal-VERSION.dmg` and drag ELYTerminal into Applications. The app is signed with Developer ID and notarized.
- Windows 10 or later, x86_64: unzip `ELYTerminal-VERSION-windows-x86_64.zip` and run `elyterminal.exe`, the zip's other files kept beside it. It is unsigned; if SmartScreen stops it, press More info, then Run anyway.
- Linux, x86_64: [packages](docs/en_us/install.md).

## Linux window reuse

On Linux, launching ELYTerminal again adds a session to the most recently active terminal window.
The first invocation owns a socket in a private directory under `XDG_RUNTIME_DIR`.
An invocation waits for the owner to create its session and reports a failed launch to stderr.
Windows and macOS keep their existing startup behavior.

```sh
elyterminal                                      # a session in the caller's directory
elyterminal --working-directory '/path/to/folder' # an explicit directory
elyterminal --new-window                         # a separate window
elyterminal -e htop                               # a program in the reused window
elyterminal -e 'printf hello; sleep 5'             # one string runs through /bin/sh -c
```

`--workdir` and `--directory` are aliases for `--working-directory`; each accepts `=PATH` too.
With `-e` or `--command`, multiple arguments name a program and its literal arguments.
The program's exit closes its session. The in-window new-session shortcut still follows Settings › Terminal › Start in.
Desktop activation tokens travel with each request, so KDE Wayland can raise the existing window.
The desktop entry and Dolphin can call the binary directly; no fish startup hook is required.

## Detachable sessions

Drag a session row from the left sidebar beyond any edge of its window to give
that live session a window of its own. The process, output history, working
directory and session name stay with it. Dragging the only session moves its
existing window instead. Press Escape before crossing the edge to cancel;
releasing inside the window keeps the session there. The close button and
in-place rename field keep their existing behavior.

## What it does

- Sessions sit on the left, the terminal in the middle, Git or Files on the right.
- Shell integration for zsh, bash, fish and PowerShell, so Files and Git follow the session's folder.
- Ely peeks over the window's edge and follows the sessions.
- Images through kitty's graphics protocol, animations too, and the clipboard through kitty's OSC 5522, which asks you before a program reads what it holds, unless you allowed that program for the session or it answers a paste.
- Background opacity and blur on macOS, Windows, Wayland and X11, and Mica on Windows 11.

The rest is in [docs/en_us/features.md](docs/en_us/features.md).

## Keys

Settings › Terminal lists them all, as this platform binds them. The table is in [docs/en_us/keys.md](docs/en_us/keys.md).

## Settings

The settings window saves each change at once to `settings.json` in the platform's config folder:

- macOS: `~/Library/Application Support/ELYTerminal/settings.json`
- Linux: `$XDG_CONFIG_HOME/ELYTerminal/settings.json`, else `~/.config/ELYTerminal/settings.json`
- Windows: `%APPDATA%\ELYTerminal\settings.json`

A file that does not parse, or holds a value out of range, stops the start with an error naming the file and the field. The app never overwrites it.

Settings › Appearance › Language shows the interface in English or 简体中文.

Settings › Terminal checks for a newer release at start and once a day, or at once with Check now; a newer one shows a Download button that opens its page. The check runs `curl`, which macOS and Windows ship.

## Build

Rust 1.95.0, which `rust-toolchain.toml` pins; rustup installs it on the first build.

```sh
cargo run --release
```

Release scripts, Linux libraries and the log are in [docs/en_us/develop.md](docs/en_us/develop.md).

## License

MIT, in `LICENSE`. Box drawing and key encoding follow Ghostty's code, MIT, in `LICENSE-GHOSTTY`. The theme files come from iTerm2-Color-Schemes, MIT, with its license in `assets/themes`. JetBrains Mono and its Nerd Font build are under the SIL Open Font License, in `assets/fonts`. The compound text decoder's tables come from libX11, under SunSoft's and Bruno Haible's notice, in `vendor/xim-ctext/tables/LICENSE`. Microsoft's ConPTY, `conpty.dll` and `OpenConsole.exe`, is MIT, in `vendor/conpty/LICENSE`.
