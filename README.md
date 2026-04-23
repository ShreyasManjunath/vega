# vega

`vega` is a rofi-like launcher prototype written in Rust. It currently uses a custom GUI plus the system-installed `fzf` binary as a managed search backend.

The name `vega` comes from Sanskrit `vēgah` (`वेग:`), meaning speed, velocity, force, or impetus.

The project is aimed at a fast, maintainable launcher with explicit search behavior, structured candidates, and a clear path toward a more native Wayland-focused architecture.

## Current Status

The repository contains a working prototype with:

- a graphical launcher window built with `eframe`/`egui`
- a refined header row with a dedicated mode badge and larger input typography
- `drun`, `run`, and `dmenu` modes
- managed `fzf` integration through `std::process::Command`
- cancellation of superseded in-flight GUI queries
- typed backend and mode errors
- conservative, shell-free execution for launched commands

This is still an early implementation. The GUI, theme system, config system, and overall module layout are expected to evolve.

## Naming Note

`vega` is a good fit semantically because of its Sanskrit meaning around speed and momentum, but it is not a unique name in software. There are already obvious public uses of `Vega`, including visualization software and an existing `vega` CLI in Amazon's Vega SDK.

If this project is intended for public release, treat the current name as provisional until you do a proper package, namespace, and trademark review for the channels you plan to ship through.

## Modes

- `drun`: load desktop applications from XDG application directories
- `run`: load executables from `PATH`
- `dmenu`: load newline-separated candidates from `stdin`

Current matching policy:

- `run` and `dmenu` use `fzf` fuzzy matching on the primary label
- `drun` fuzzy matching uses desktop `Name`
- `drun` `GenericName` participates in direct exact, prefix, and substring matches before fuzzy fallback
- `drun` `Comment` is intentionally excluded from matching

Use `--debug` to print the configured `fzf` binary and the resolved executable path, so system `fzf` usage is visible during runs.

## Build And Run

Requirements:

- Rust and Cargo
- `fzf` installed on `PATH`

Build:

```bash
cargo build
```

Run the GUI:

```bash
cargo run -- -show run
cargo run -- -show drun
printf 'Firefox\nFiles\nTerminal\n' | cargo run -- -show dmenu
```

Run non-interactively:

```bash
cargo run -- -show run --query alacritty
cargo run -- -show drun --query browser
printf 'Firefox\nFiles\nTerminal\n' | cargo run -- -show dmenu --query fire
```

Use `--execute` to launch the first non-interactive match instead of printing it.

## Development

Common commands:

```bash
cargo fmt
cargo test
cargo clippy --all-targets -- -D warnings
```

Pre-commit hooks are configured with [`pre-commit`](https://pre-commit.com/) in [`.pre-commit-config.yaml`](./.pre-commit-config.yaml).

GitHub Actions mirror this split with separate workflows for pre-commit-style checks and build/test validation under [`.github/workflows/`](./.github/workflows).

Install them locally with:

```bash
pre-commit install
pre-commit install --hook-type pre-push
```

The configured hooks currently run:

- `pre-commit`: file hygiene checks, Markdown formatting, `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`
- `pre-push`: `cargo test`

Style conventions are documented in [docs/coding-style.md](./docs/coding-style.md).

Automated release versioning is configured with [`.releaserc.yml`](./.releaserc.yml) for semantic-release on `main`.

## License

`vega` is licensed under the [MIT License](./LICENSE).

Third-party dependency licensing is summarized in
[THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md). The current dependency
graph is mostly `MIT` / `Apache-2.0`, but some transitive crates also carry
additional permissive notice-style licenses such as Unicode, BSD, Zlib, and
bundled font terms.

## Design Notes

`vega` does not use shell pipelines such as `rofi | fzf | rofi`. The Rust code owns:

- candidate loading
- match policy
- `fzf` process lifecycle
- query cancellation and cleanup
- result parsing
- timeout handling
- execution

Deeper documentation:

- [Architecture](./docs/architecture.md)
- [Development Notes](./docs/dev-notes.md)
- [fzf Backend](./docs/fzf-backend.md)
