# Notes

## Behavior

- The GUI is built in `src/gui.rs` with GTK4.
- Configuration is loaded from the XDG config directory and merged over built-in defaults.
- Theme files use a CSS-like section/property syntax, and templates use MiniJinja.
- Query execution is asynchronous and uses generation checks plus explicit cancellation to suppress stale results.
- The GUI hot-reloads config, theme, and template changes by polling the config tree.
- Wayland: `gtk4-layer-shell` (optional Cargo feature, default on) positions the window as a centered overlay. Requires the `gtk4-layer-shell` C library. Binary still runs without it — layer-shell is skipped at runtime via GDK display type check.
- X11: `gdk4-x11` (optional Cargo feature, default on) centers the window via `XMoveWindow` via idle callback after map.
- The result list supports hover highlight, single-click selection, and double-click execution.
- Non-interactive mode reuses the same backend path as the GUI and can either print or execute the first match.

Supported modes (`-show <mode>`):

- `dmenu`: reads newline-separated candidates from stdin.
- `cmd`: scans executable files from `PATH`.
- `apps`: scans `.desktop` files from XDG application directories.

## Search And Execution

Search policy:

- `cmd` and `dmenu`: managed `fzf --filter` matching on candidate primary labels.
- `apps`: exact → prefix → substring on Name/GenericName first; fuzzy fallback through `fzf` only when no direct match.
- `apps` comments intentionally excluded from matching.

Execution:

- `cmd`: executes direct executable paths through `Command`.
- `apps`: parses desktop `Exec` lines into argv, strips field codes such as `%u`, rejects direct shell interpreters such as `sh` or `bash`.

## Commands

```bash
cargo fmt
cargo test
cargo clippy --all-targets -- -D warnings
cargo run -- -show cmd
cargo run -- -show apps
cargo run -- -show cmd --query alacritty
printf 'Firefox\nFiles\nTerminal\n' | cargo run -- -show dmenu --query fire

# feature-specific builds
cargo build --no-default-features --features x11      # X11 only, no layer-shell
cargo build --no-default-features --features layer-shell  # Wayland layer-shell only
cargo build                                            # both (default)
```

Omit `--query` to open the GUI. Use `--query` for non-interactive filtering. Add `--execute` to launch the first match instead of printing it.

Use `--debug` to print:

- the configured `fzf` binary
- the resolved path from `PATH`
- mode/query result counts and elapsed backend time

## System Dependencies

GTK4 and gtk4-layer-shell must be installed before building.

| Distro               | Command                                                         |
| -------------------- | --------------------------------------------------------------- |
| Arch                 | `pacman -S gtk4 gtk4-layer-shell`                               |
| Ubuntu 24.10+        | `apt install libgtk-4-dev libgtk4-layer-shell-dev`              |
| Ubuntu 22.04 / 24.04 | `apt install libgtk-4-dev` + build gtk4-layer-shell from source |
| Fedora               | `dnf install gtk4-devel gtk4-layer-shell-devel`                 |

Build gtk4-layer-shell from source (Ubuntu 22.04):

```bash
sudo apt install meson ninja-build libwayland-dev wayland-protocols
git clone --depth=1 --branch v1.0.3 https://github.com/wmww/gtk4-layer-shell.git
meson setup gtk4-layer-shell/build gtk4-layer-shell -Dexamples=false -Ddocs=false -Dtests=false -Dintrospection=false --prefix=/usr
sudo ninja -C gtk4-layer-shell/build install
sudo ldconfig
```

To build without layer-shell (e.g. on a pure X11 system):

```bash
cargo build --no-default-features --features x11
```

## CI

CI runs on `ubuntu-latest`. As of April 29, 2026, GitHub Actions `ubuntu-latest` resolves to Ubuntu 24.04, where `libgtk4-layer-shell-dev` is still unavailable, so the source-build fallback is the normal path. The install script at `.github/scripts/install-gtk-deps.sh`:

1. Installs `libgtk-4-dev` and `pkg-config` via apt (available on 22.04+).
1. Attempts `apt install libgtk4-layer-shell-dev` (works on Ubuntu 24.10+ with universe; falls back on Ubuntu 22.04 and 24.04).
1. On failure, builds gtk4-layer-shell v1.0.3 from source using meson + ninja with boolean `false` Meson options and runs `ldconfig`.

All three CI workflows (pre-commit, build, release) call this script before any `cargo` invocation so fmt/clippy/test all compile against the full default feature set.

## Development

- `pre-commit`: file hygiene checks, Markdown formatting, `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`
- `pre-push`: `cargo test`

CI runs separate workflows for linting, build/test validation, and release automation.

## Open Work

- broader integration tests, especially around fake `fzf` processes and desktop-entry edge cases
- benchmarking before any move to a persistent `fzf` process
- eventual decomposition of `src/gui.rs` into the fuller module structure described in `docs/architecture.md`
- verify X11 centering on window managers that defer placement (e.g. i3, openbox)
- Ubuntu packaging for `gtk4-layer-shell` C library dependency
