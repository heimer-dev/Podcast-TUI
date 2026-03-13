#!/usr/bin/env bash
set -e

BINARY_NAME="podi"
INSTALL_DIR="$HOME/.local/bin"

echo "==> PodcastTUI Installer"
echo ""

# Check mpv
if ! command -v mpv &>/dev/null; then
    echo "[!] mpv nicht gefunden. Bitte installieren:"
    echo "    Arch:   sudo pacman -S mpv"
    echo "    Ubuntu: sudo apt install mpv"
    echo "    macOS:  brew install mpv"
    exit 1
fi
echo "[✓] mpv gefunden: $(mpv --version | head -1)"

# Check / install Rust
if ! command -v cargo &>/dev/null; then
    if [ -f "$HOME/.cargo/env" ]; then
        source "$HOME/.cargo/env"
    fi
fi

if ! command -v cargo &>/dev/null; then
    echo "[~] Rust nicht gefunden — installiere rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
    source "$HOME/.cargo/env"
fi
echo "[✓] cargo gefunden: $(cargo --version)"

# Build
echo ""
echo "[~] Baue Release-Binary (das kann einen Moment dauern)..."
cargo build --release

# Install
mkdir -p "$INSTALL_DIR"
cp -f "target/release/podcast-tui" "$INSTALL_DIR/$BINARY_NAME"
chmod +x "$INSTALL_DIR/$BINARY_NAME"

echo ""
echo "[✓] Installiert: $INSTALL_DIR/$BINARY_NAME"

# PATH hint
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo ""
    echo "[!] $INSTALL_DIR ist nicht in deinem PATH."
    echo "    Füge folgendes zu deiner Shell-Config hinzu:"
    echo ""
    echo "    bash/zsh:  export PATH=\"\$HOME/.local/bin:\$PATH\""
    echo "    fish:      fish_add_path ~/.local/bin"
    echo ""
fi

echo "[✓] Fertig! Starte mit: podi"
