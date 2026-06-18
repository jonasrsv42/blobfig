#!/usr/bin/env bash
#
# Install the `blobfig` CLI into your user bin directory ($HOME/.local/bin by
# default) so you can run `blobfig <file>` from anywhere.
#
# Usage:
#   ./install.sh              # installs to $HOME/.local/bin
#   PREFIX=/some/where ./install.sh   # installs to /some/where/bin
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"
BIN_DIR="$PREFIX/bin"

mkdir -p "$BIN_DIR"

echo "Building and installing the blobfig CLI to $BIN_DIR ..."
# `cargo install --root` places binaries in <root>/bin. The package is
# `blobfig-cli` but its binary is named `blobfig`.
cargo install --path "$SCRIPT_DIR" --root "$PREFIX" --force

echo
echo "Installed: $BIN_DIR/blobfig"

case ":$PATH:" in
    *":$BIN_DIR:"*)
        echo "Run: blobfig <file.blobfig>"
        ;;
    *)
        echo "Note: $BIN_DIR is not on your PATH. Add this to your shell profile:"
        echo "    export PATH=\"$BIN_DIR:\$PATH\""
        ;;
esac
