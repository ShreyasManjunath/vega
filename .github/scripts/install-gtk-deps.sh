#!/usr/bin/env bash
# Install GTK4 system dependencies for vega CI.
# Tries apt first (Ubuntu 24.04+ may have libgtk4-layer-shell-dev).
# Falls back to building gtk4-layer-shell from source on older images.
set -euo pipefail

sudo apt-get update -q
sudo apt-get install -y libgtk-4-dev pkg-config

# Try apt. If the package isn't in the index, fall back to source build.
if sudo apt-get install -y libgtk4-layer-shell-dev >/dev/null 2>&1; then
    echo "gtk4-layer-shell installed via apt"
else
    echo "libgtk4-layer-shell-dev not in apt — building from source"
    sudo apt-get install -y meson ninja-build libwayland-dev wayland-protocols

    git clone --depth=1 --branch v1.0.3 \
        https://github.com/wmww/gtk4-layer-shell.git /tmp/gtk4-layer-shell

    meson setup /tmp/gtk4-layer-shell/build /tmp/gtk4-layer-shell \
        -Dexamples=false -Ddocs=false -Dtests=false --prefix=/usr

    sudo ninja -C /tmp/gtk4-layer-shell/build install
    sudo ldconfig
fi
