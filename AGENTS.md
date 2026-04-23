# Repository Guidelines

## Project Structure & Module Organization

This repository contains a Rust CLI/backend prototype for a rofi-like Wayland launcher. Keep architectural decisions synchronized between `README.md`, `docs/architecture.md`, and the rest of `docs/` when requirements change. Current code is organized as:

- `src/main.rs` for CLI parsing and mode dispatch.
- `src/candidate.rs` for structured candidate IDs, labels, metadata, and actions.
- `src/fzf.rs` for managed fzf process execution, transport, parsing, and errors.
- `src/modes.rs` for `dmenu`, `run`, and `drun` providers.
- `docs/` for backend and development notes.

Future graphical implementation should extend the documented layout:

- `src/app/` for controller and application state.
- `src/ui/` for Wayland windowing, rendering, input, and theme application.
- `src/search/` for the fzf-backed search interface.
- `src/modes/` for `drun`, `run`, `dmenu`, `window`, and `keys` providers.
- `src/config/`, `src/cache/`, `src/platform/`, and `src/diagnostics/` for cross-cutting concerns.

## Build, Test, and Development Commands

Use standard Cargo commands:

- `cargo build` builds a debug binary.
- `cargo build --release` builds an optimized launcher.
- `cargo test` runs unit and integration tests.
- `cargo fmt` formats Rust code.
- `cargo clippy --all-targets -- -D warnings` checks code quality before review.

Document any new required runtime dependency, especially `fzf` and Wayland-related libraries.

## Coding Style & Naming Conventions

Prefer explicit, small modules with clear ownership boundaries. Keep UI, controller, search backend, and mode/provider logic separate. Rust code should use the repository formatting and style files (`rustfmt.toml`, `.editorconfig`, and `docs/coding-style.md`), `snake_case` for functions/modules, `PascalCase` for types, and descriptive error enums instead of string-only failures. Avoid shell pipelines for backend behavior; fzf integration must be controlled through structured process management.

## Testing Guidelines

Prioritize tests for matching/backend integration, candidate serialization, duplicate labels, cancellation, timeouts, config parsing, and mode execution behavior. Place unit tests near the module under test and broader behavior tests under `tests/` when that directory is added. Include failure-path tests for missing `fzf`, broken pipes, invalid output, and subprocess cleanup.

## Commit & Pull Request Guidelines

Git history is not available in this workspace, so no repository-specific commit convention can be inferred. Use concise imperative commit messages such as `Add fzf backend lifecycle design` or `Implement candidate ID mapping`. Pull requests should include a short behavior summary, relevant test results, linked issues if available, and screenshots or recordings for UI changes.

## Agent-Specific Instructions

Before editing architecture-sensitive code, read `docs/architecture.md`. Preserve the product constraints: Wayland-native UI, fzf as a managed backend component for v1, structured candidates, responsive UI, clear diagnostics, and no fragile shell chaining.
