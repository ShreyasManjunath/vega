# Coding Style

This project uses a small set of explicit conventions so formatting and review stay predictable.

## Source Formatting

- Rust formatting is handled by `rustfmt`.
- Use the repository `rustfmt.toml`.
- Keep files ASCII by default unless existing content or user-facing text requires Unicode.
- Preserve LF line endings.

## General Style

- Prefer small modules with explicit ownership boundaries.
- Keep UI, search/backend, and mode/provider logic separate.
- Use descriptive enums and typed errors instead of stringly-typed control flow.
- Avoid shell pipelines for launcher behavior; use structured Rust process management.
- Keep comments sparse and high-signal.

## Naming

- `snake_case` for functions, modules, and variables
- `PascalCase` for structs and enums
- descriptive names over abbreviations unless the domain term is already standard

## Rust-Specific Expectations

- Run `cargo fmt` before commit.
- Run `cargo clippy --all-targets -- -D warnings` before commit.
- Add tests for behavior changes, especially around matching, cancellation, parsing, and subprocess lifecycle.
- Prefer unit tests near the module under test; move broader behavior tests into `tests/` when that tree exists.

## Hooks

This repository includes a `pre-commit` configuration:

- `pre-commit`: file hygiene checks, Markdown formatting, `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`
- `pre-push`: `cargo test`

Install with:

```bash
pre-commit install
pre-commit install --hook-type pre-push
```
