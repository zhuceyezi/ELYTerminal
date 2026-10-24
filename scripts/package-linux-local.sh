#!/bin/sh
# Package an already-built native Linux binary without Docker.
set -eu
cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)
out="target/release/ELYTerminal-$version-linux-window-reuse-x86_64"
mkdir -p "$out/scripts"
cp target/release/elyterminal "$out/elyterminal"
cp assets/icons/elyterminal.svg "$out/com.zacharyzhang.elyterminal.svg"
cp scripts/install-linux.py "$out/scripts/install-linux.py"
cp LICENSE LICENSE-GHOSTTY README.md UPSTREAM.md "$out/"
cat > "$out/com.zacharyzhang.elyterminal.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=ELYTerminal
Exec=elyterminal
Icon=com.zacharyzhang.elyterminal
Terminal=false
Categories=System;TerminalEmulator;
StartupWMClass=com.zacharyzhang.elyterminal
StartupNotify=true
X-KDE-Shortcuts=Ctrl+Alt+T,Meta+Return
EOF
tar -C target/release -czf "$out.tar.gz" "$(basename "$out")"
printf '%s\n' "$out.tar.gz"
