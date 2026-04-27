# vega

`vega` is a Rust launcher with a custom GUI and an `fzf` backend.

The name `vega` comes from Sanskrit `vēgah` (`वेग:`), meaning speed, velocity, force, or impetus.

The GUI runs through `eframe`/`egui` on top of `winit`, with Wayland and X11 support.

## Features

- GUI launcher with keyboard navigation, hover feedback, click selection, and async query handling
- `apps`, `cmd`, and `dmenu` modes
- `fzf` integration through `std::process::Command`
- structured candidates and typed backend/mode errors
- shell-free execution for launched commands
- Wayland and X11 support through the current windowing stack

## Naming Note

`vega` comes from Sanskrit `vēgah` (`वेग:`), meaning speed, velocity, force, or impetus. The name is not unique in software, so public packaging and naming should be reviewed before release.

## Modes

- `apps`: load desktop applications from XDG application directories
- `cmd`: load executables from `PATH`
- `dmenu`: load newline-separated candidates from `stdin`

Matching policy:

- `cmd` and `dmenu` use `fzf` fuzzy matching on the primary label
- `apps` fuzzy matching uses desktop `Name`
- `apps` `GenericName` participates in direct exact, prefix, and substring matches before fuzzy fallback
- `apps` `Comment` is intentionally excluded from matching

Use `--debug` to print the configured `fzf` binary, the resolved executable path, and per-query backend diagnostics.

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

Use `--execute` to launch the first non-interactive match instead of printing it.

## Development

Common commands:

```bash
cargo fmt
cargo test
cargo clippy --all-targets -- -D warnings
```

Pre-commit hooks are configured in [`.pre-commit-config.yaml`](./.pre-commit-config.yaml).

GitHub Actions mirror this split with separate workflows for `pre-commit`, build/test validation, and release automation under [`.github/workflows/`](./.github/workflows).

Install them locally with:

```bash
pre-commit install
pre-commit install --hook-type pre-push
```

Configured hooks:

- `pre-commit`: file hygiene checks, Markdown formatting, `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`
- `pre-push`: `cargo test`

Style conventions are documented in [docs/coding-style.md](./docs/coding-style.md).

Release automation is configured with [`.releaserc.yml`](./.releaserc.yml).

## License

`vega` is licensed under the [MIT License](./LICENSE).

Third-party dependency licensing is summarized in [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md).

## Documentation

`vega` does not use shell pipelines such as `rofi | fzf | rofi`.

Documentation:

- [Architecture](./docs/architecture.md)
- [Notes](./docs/dev-notes.md)
- [fzf Backend](./docs/fzf-backend.md)
