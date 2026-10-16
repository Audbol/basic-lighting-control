#!/bin/sh
# Installs Obey-Lite for the current user: the binary in ~/.local/bin and a
# menu entry. Run from the unpacked release folder: ./install.sh
set -e
here=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications"
install -m 755 "$here/obey-lite" "$HOME/.local/bin/obey-lite"
sed "s|^Exec=.*|Exec=$HOME/.local/bin/obey-lite|" "$here/obey-lite.desktop" \
  > "$HOME/.local/share/applications/obey-lite.desktop"
echo "Installed. Start it from the menu or run: $HOME/.local/bin/obey-lite"
