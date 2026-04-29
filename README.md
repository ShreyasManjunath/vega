# vega

`vega` is a Rust launcher with a GTK4 GUI and an `fzf` backend.

It supports Wayland through `gtk4-layer-shell` and X11 through `gdk4-x11`. Search and execution stay in Rust code; `fzf` is used as a managed backend, not as part of a shell pipeline.

The name `vega` comes from Sanskrit `vēgah` (`वेग:`), meaning speed, velocity, force, or impetus.

## Features

- GTK4 GUI with keyboard navigation, hover feedback, click selection, and async query handling
- `apps`, `cmd`, and `dmenu` modes
- `fzf` integration through `std::process::Command`
- layered XDG configuration with TOML parsing through `serde`
- CSS-like theming with built-in `catppuccin-mocha` and `gruvbox-dark` themes
- optional MiniJinja templates for badge, row, and empty-state rendering
- hot-reload for config, theme, and template changes
- structured candidates and typed backend and mode errors
- shell-free execution for launched commands

## Installation

You need:

- Rust and Cargo
- `fzf` on `PATH`
- GTK4 development libraries
- `gtk4-layer-shell` if you want Wayland layer-shell overlay support

### Arch And Similar

Install dependencies:

```bash
sudo pacman -S --needed \
  rust \
  fzf \
  gtk4 \
  gtk4-layer-shell \
  gnu-free-fonts \
  base-devel \
  pkgconf \
  glib2 \
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
```

### Ubuntu, Debian, And Similar

Install dependencies:

```bash
sudo apt-get update
sudo apt-get install -y \
  cargo \
  rustc \
  fzf \
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
```

Then try to install the Wayland layer-shell development package:

```bash
sudo apt-get install -y libgtk4-layer-shell-dev
```

If that package is unavailable on your release, build `gtk4-layer-shell` from source:

```bash
git clone --depth=1 --branch v1.0.3 https://github.com/wmww/gtk4-layer-shell.git /tmp/gtk4-layer-shell
meson setup /tmp/gtk4-layer-shell/build /tmp/gtk4-layer-shell \
  --prefix=/usr \
  -Dexamples=false \
  -Ddocs=false \
  -Dtests=false \
  -Dintrospection=false \
  -Dvapi=false
sudo ninja -C /tmp/gtk4-layer-shell/build install
sudo ldconfig
```

### Other Distros

Install the equivalents of these packages with your package manager:

- Rust toolchain
- search backend: `fzf`
- GTK4 development files
- `gtk4-layer-shell` development files if you want Wayland layer-shell support
- at least one TrueType font package if your distro treats GTK font dependencies as virtual providers
- build tools: `pkg-config`, `git`, `meson`, `ninja`
- Wayland development files: `wayland`, `wayland-protocols`
- X11 development files: `libx11`, `libxext`, `libxrandr`, `libxfixes`, `libxi`, `libxinerama`, `libxcursor`
- introspection and Vala tools: `gobject-introspection`, `libgirepository`, `vala`

If you do not need Wayland layer-shell support, build without the default `layer-shell` feature instead of installing `gtk4-layer-shell`.

## Build

Default build:

```bash
cargo build
```

X11-only build:

```bash
cargo build --no-default-features --features x11
```

Wayland layer-shell only:

```bash
cargo build --no-default-features --features layer-shell
```

## Run

Run the GUI:

```bash
cargo run -- -show cmd
cargo run -- -show apps
printf 'Firefox\nFiles\nTerminal\n' | cargo run -- -show dmenu
```

Run non-interactively:

```bash
cargo run -- -show cmd --query alacritty
cargo run -- -show apps --query browser
printf 'Firefox\nFiles\nTerminal\n' | cargo run -- -show dmenu --query fire
```

Add `--execute` to run the first non-interactive match instead of printing it.

Use `--debug` to print the configured `fzf` binary, the resolved executable path, and backend timing and result diagnostics.

## Modes

- `apps`: load desktop applications from XDG application directories
- `cmd`: load executables from `PATH`
- `dmenu`: load newline-separated candidates from `stdin`

Matching policy:

- `cmd` and `dmenu` use `fzf` fuzzy matching on primary labels
- `apps` uses exact, prefix, and substring matches on `Name` and `GenericName` before fuzzy fallback
- `apps` fuzzy fallback uses desktop `Name`
- `apps` excludes desktop `Comment` from matching

## Configuration

`vega` reads user settings from the XDG config directory:

- `~/.config/vega/config.toml`
- `~/.config/vega/themes/<name>.theme`
- `~/.config/vega/templates/<template-name>.*`

Built-in themes:

- `catppuccin-mocha`
- `gruvbox-dark`

Configuration is layered as built-in defaults followed by user overrides. The GUI hot-reloads the active config file, the active theme chain, and top-level template files.

Theme packs can be cloned directly into `~/.config/vega/themes/`. `vega` resolves:

- direct files such as `my-theme.theme`
- repo-style directories with entry files such as `vega.theme`, `theme.theme`, or `index.theme`
- nested theme names such as `collection/gruvbox-dark`

See [docs/configuration.md](./docs/configuration.md) for the full configuration format.

## Development

Common commands:

```bash
cargo fmt
cargo test
cargo clippy --all-targets -- -D warnings
```

Pre-commit hooks are configured in [`.pre-commit-config.yaml`](./.pre-commit-config.yaml). Install them with:

```bash
pre-commit install
pre-commit install --hook-type pre-push
```

Configured hooks:

- `pre-commit`: file hygiene checks, Markdown formatting, `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`
- `pre-push`: `cargo test`

GitHub Actions use three workflows under [`.github/workflows/`](./.github/workflows):

- `pre-commit`: lint and formatting checks
- `build`: `cargo test` and `cargo build --release`
- `release`: semantic-release automation

Style conventions are documented in [docs/coding-style.md](./docs/coding-style.md).

## Documentation

- [Architecture](./docs/architecture.md)
- [Configuration](./docs/configuration.md)
- [Notes](./docs/dev-notes.md)
- [fzf Backend](./docs/fzf-backend.md)

## License

`vega` is licensed under the [MIT License](./LICENSE).

Third-party dependency licensing is summarized in [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md).
