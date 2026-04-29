#!/usr/bin/env bash
# Install GTK4 + X11/Wayland build dependencies for vega.
# Supports Ubuntu/Debian (apt) and Arch (pacman).
# On Ubuntu 22.04, gtk4-layer-shell is built from source as a fallback.
set -euo pipefail

if command -v apt-get >/dev/null 2>&1; then
    sudo apt-get update -q
    sudo apt-get install -y \
        pkg-config \
        build-essential \
        git \
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
        valac
    if sudo apt-get install -y libgtk4-layer-shell-dev >/dev/null 2>&1; then
        echo "gtk4-layer-shell installed via apt"
    else
        echo "libgtk4-layer-shell-dev not available via apt; building gtk4-layer-shell from source"
        sudo apt-get install -y meson ninja-build

        git clone --depth=1 --branch v1.0.3 \
            https://github.com/wmww/gtk4-layer-shell.git /tmp/gtk4-layer-shell

        meson setup /tmp/gtk4-layer-shell/build /tmp/gtk4-layer-shell \
            -Dexamples=false -Ddocs=false -Dtests=false -Dintrospection=false --prefix=/usr

        sudo ninja -C /tmp/gtk4-layer-shell/build install
        sudo ldconfig
    fi
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
else
    echo "Unsupported package manager. Please install GTK4, gtk4-layer-shell, Wayland, X11, and pkg-config development packages manually."
    exit 1
fi
