#!/usr/bin/env bash
set -euo pipefail

GTK4_LAYER_SHELL_VERSION="${GTK4_LAYER_SHELL_VERSION:-v1.0.3}"
GTK4_LAYER_SHELL_DIR="/tmp/gtk4-layer-shell"

if command -v apt-get >/dev/null 2>&1; then
    sudo apt-get update -q

    sudo apt-get install -y \
        pkg-config \
        build-essential \
        git \
        meson \
        ninja-build \
        libglib2.0-dev \
        libgtk-4-dev \
        libwayland-dev \
        wayland-protocols \
        libx11-dev \
        libxext-dev \
        libxrandr-dev \
        libxfixes-dev \
        libxi-dev \
        libxinerama-dev \
        libxcursor-dev \
        gobject-introspection \
        libgirepository1.0-dev \
        valac

    if sudo apt-get install -y libgtk4-layer-shell-dev >/dev/null 2>&1; then
        echo "gtk4-layer-shell installed via apt"
    else
        echo "libgtk4-layer-shell-dev not available via apt; building from source"

        rm -rf "$GTK4_LAYER_SHELL_DIR"

        git clone --depth=1 --branch "$GTK4_LAYER_SHELL_VERSION" \
            https://github.com/wmww/gtk4-layer-shell.git \
            "$GTK4_LAYER_SHELL_DIR"

        meson setup "$GTK4_LAYER_SHELL_DIR/build" "$GTK4_LAYER_SHELL_DIR" \
            --prefix=/usr \
            -Dexamples=false \
            -Ddocs=false \
            -Dtests=false \
            -Dintrospection=false \
            -Dvapi=false

        sudo ninja -C "$GTK4_LAYER_SHELL_DIR/build" install
        sudo ldconfig
    fi

    pkg-config --modversion gtk4-layer-shell-0

elif command -v pacman >/dev/null 2>&1; then
    sudo pacman -Syu --noconfirm

    sudo pacman -S --needed --noconfirm \
        base-devel \
        pkgconf \
        git \
        glib2 \
        gtk4 \
        gtk4-layer-shell \
        wayland \
        wayland-protocols \
        libx11 \
        libxext \
        libxrandr \
        libxfixes \
        libxi \
        libxinerama \
        libxcursor \
        gobject-introspection \
        vala

    pkgconf --modversion gtk4-layer-shell-0

else
    echo "Unsupported package manager."
    echo "Install GTK4, gtk4-layer-shell, Wayland, X11, and pkg-config development packages manually."
    exit 1
fi
