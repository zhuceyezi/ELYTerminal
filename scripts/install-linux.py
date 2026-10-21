#!/usr/bin/env python3
"""Install a packaged ELYTerminal build for the current user."""
import datetime
import json
import os
from pathlib import Path
import shutil
import subprocess

source = Path(__file__).resolve().parent.parent
home = Path.home()
backup = home / '.local/share/ELYTerminal/backups' / datetime.datetime.now().strftime('%Y%m%d-%H%M%S-%f')
backup.mkdir(parents=True)
files = {
    'binary': home / '.local/lib/elyterminal/elyterminal',
    'launcher': home / '.local/bin/elyterminal',
    'desktop': home / '.local/share/applications/com.zacharyzhang.elyterminal.desktop',
    'fish-hook': home / '.config/fish/conf.d/elyterminal-kde.fish',
}
manifest = {}
for name, path in files.items():
    if path.exists():
        shutil.copy2(path, backup / name)
        manifest[name] = str(path)
(backup / 'manifest.json').write_text(json.dumps(manifest, indent=2))
binary = files['binary']
binary.parent.mkdir(parents=True, exist_ok=True)
staged = binary.with_name('elyterminal.new')
shutil.copy2(source / 'elyterminal', staged)
staged.chmod(0o755)
os.replace(staged, binary)
launcher = files['launcher']
launcher.parent.mkdir(parents=True, exist_ok=True)
launcher.write_text('#!/bin/sh\nexec "$HOME/.local/lib/elyterminal/elyterminal" "$@"\n')
launcher.chmod(0o755)
desktop = files['desktop']
desktop.parent.mkdir(parents=True, exist_ok=True)
text = (source / 'com.zacharyzhang.elyterminal.desktop').read_text()
text = text.replace('Exec=elyterminal\n', f'Exec="{launcher}"\n')
desktop.write_text(text)
icon = home / '.local/share/icons/hicolor/scalable/apps/com.zacharyzhang.elyterminal.svg'
icon.parent.mkdir(parents=True, exist_ok=True)
shutil.copy2(source / 'com.zacharyzhang.elyterminal.svg', icon)
files['fish-hook'].unlink(missing_ok=True)
for command in [('kbuildsycoca6', '--noincremental'), ('update-desktop-database', str(desktop.parent))]:
    if shutil.which(command[0]):
        subprocess.run(command, check=True)
print('Installed:', binary)
print('Backup:', backup)
print('Existing settings, terminal defaults, and shortcut assignments are preserved.')
