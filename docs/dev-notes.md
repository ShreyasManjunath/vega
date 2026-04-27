# Notes

## Behavior

- The GUI is built in `src/gui.rs` with `eframe`/`egui`.
- Configuration is loaded from the XDG config directory and merged over built-in defaults.
- Theme files use a CSS-like section/property syntax, and templates use MiniJinja.
- Query execution is asynchronous and uses generation checks plus explicit cancellation to suppress stale results.
- The GUI hot-reloads config, theme, and template changes by polling the config tree.
- The runtime stack works on both Wayland and X11 through `winit`.
- The result list supports hover highlight, single-click selection, and double-click execution.
- Non-interactive mode reuses the same backend path as the GUI and can either print or execute the first match.

Supported modes:

- `dmenu`: reads newline-separated candidates from stdin.
- `cmd`: scans executable files from `PATH`.
- `apps`: scans `.desktop` files from XDG application directories.

## Search And Execution

Search policy:

- `cmd` and `dmenu` use managed `fzf --filter` matching on candidate primary labels.
- `apps` searches desktop application names with fuzzy fallback through `fzf`.
- `apps` generic names remain available for direct exact, prefix, and substring matches before fuzzy fallback.
- `apps` comments are intentionally excluded from matching.

Execution:

- `cmd` executes direct executable paths through `Command`.
- `apps` parses desktop `Exec` lines into argv, strips field codes such as `%u`, and rejects direct shell interpreters such as `sh` or `bash`.

## Commands

```bash
cargo fmt
cargo test
cargo clippy --all-targets -- -D warnings
cargo run -- -show cmd
cargo run -- -show apps
printf 'Firefox\nFiles\nTerminal\n' | cargo run -- -show dmenu --query fire
cargo run -- -show cmd --query alacritty
```

Omit `--query` to open the GUI. Use `--query` for non-interactive filtering. Add `--execute` to launch the first match instead of printing it.

Use `--debug` to print:

- the configured `fzf` binary
- the resolved path from `PATH`
- mode/query result counts and elapsed backend time

## Development

- `pre-commit`: file hygiene checks, Markdown formatting, `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`
- `pre-push`: `cargo test`

CI runs separate workflows for linting, build/test validation, and release automation.

## Open Work

- broader integration tests, especially around fake `fzf` processes and desktop-entry edge cases
- benchmarking before any move to a persistent `fzf` process
- eventual decomposition of `src/gui.rs` into the fuller module structure described in `docs/architecture.md`
