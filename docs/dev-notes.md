# Development Notes

## Current Scope

The repository now contains a Rust application named `vega`. It has a first graphical launcher window plus a production-oriented backend for validating modes, candidate modeling, and managed `fzf` integration. The UI is implemented with eframe/winit, which can run on Wayland through the platform stack.

Current GUI state:

- custom launcher window with a dedicated mode badge
- larger, more legible query input
- result list with improved row typography
- generation-based stale result suppression for async query workers
- explicit cancellation of superseded in-flight GUI queries

Supported modes:

- `dmenu`: reads newline-separated candidates from stdin.
- `run`: scans executable files from `PATH`.
- `drun`: scans `.desktop` files from XDG application directories.

Current search policy:

- `run` and `dmenu` use managed `fzf --filter` matching on candidate primary labels.
- `drun` searches desktop application names with fuzzy fallback through `fzf`.
- `drun` generic names are display-only for fuzzy matching, but still participate in direct exact/prefix/substring matches.
- `drun` comments are intentionally excluded from search to avoid surprising low-signal results.

## Commands

```bash
cargo fmt
cargo test
cargo clippy -- -D warnings
cargo run -- -show run
cargo run -- -show drun
printf 'Firefox\nFiles\nTerminal\n' | cargo run -- -show dmenu --query fire
cargo run -- -show run --query alacritty
```

Omit `--query` to open the custom launcher window. Use `--query` for non-interactive filtering in scripts. Use `--execute` only when you want to launch the first selected result from a non-interactive run. Execution uses `Command`, not a shell.

Use `--debug` to print:

- the configured `fzf` binary
- the resolved path from `PATH`
- mode/query result counts and elapsed backend time

## Safety Notes

The `run` mode launches direct executable paths. The `drun` mode parses desktop `Exec` lines into argv, strips field codes such as `%u`, rejects unquoted shell operators, and rejects direct shell interpreters such as `sh` or `bash`. Full desktop-entry compatibility is larger than this first slice and should be expanded with tests before UI integration.

## Benchmark Targets

Measure these before choosing a persistent fzf process:

- process startup overhead for `fzf --filter`;
- candidate serialization and stdin write throughput;
- stdout parse latency at 1k, 10k, and 50k candidates;
- perceived latency when query work runs off the UI thread.

## Next Implementation Steps

1. Move GUI code from `src/gui.rs` into a fuller `src/ui/` module tree as it grows.
1. Add config loading under `src/config/`.
1. Add integration tests with a fake fzf binary.
1. Benchmark restart-per-query before attempting persistent mode.
1. Consider making `drun` stricter still: exact/prefix/substring-only for `drun`, with no fuzzy fallback once the query is already a strong name hit.
