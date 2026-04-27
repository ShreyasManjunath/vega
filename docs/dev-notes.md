# Notes

## Behavior

- The GUI is built in `src/gui.rs` with `eframe`/`egui`.
- Query execution is asynchronous and uses generation checks plus explicit cancellation to suppress stale results.
- The runtime stack works on both Wayland and X11 through `winit`.
- The result list supports hover highlight, single-click selection, and double-click execution.
- Non-interactive mode reuses the same backend path as the GUI and can either print or execute the first match.

Supported modes:

- `dmenu`: reads newline-separated candidates from stdin.
- `run`: scans executable files from `PATH`.
- `drun`: scans `.desktop` files from XDG application directories.

## Search And Execution

Search policy:

- `run` and `dmenu` use managed `fzf --filter` matching on candidate primary labels.
- `drun` searches desktop application names with fuzzy fallback through `fzf`.
- `drun` generic names remain available for direct exact, prefix, and substring matches before fuzzy fallback.
- `drun` comments are intentionally excluded from matching.

Execution:

- `run` executes direct executable paths through `Command`.
- `drun` parses desktop `Exec` lines into argv, strips field codes such as `%u`, and rejects direct shell interpreters such as `sh` or `bash`.

## Commands

```bash
cargo fmt
cargo test
cargo clippy --all-targets -- -D warnings
cargo run -- -show run
cargo run -- -show drun
printf 'Firefox\nFiles\nTerminal\n' | cargo run -- -show dmenu --query fire
cargo run -- -show run --query alacritty
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

- config loading and external theme support
- broader integration tests, especially around fake `fzf` processes and desktop-entry edge cases
- benchmarking before any move to a persistent `fzf` process
- eventual decomposition of `src/gui.rs` into the fuller module structure described in `docs/architecture.md`
