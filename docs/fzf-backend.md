# fzf Backend

`vega` treats `fzf` as a managed search backend, not as a loose shell pipeline. The Rust code owns process creation, candidate serialization, result parsing, timeout handling, error reporting, and mode-specific pre-filter behavior.

## Process Model

The v1 implementation uses a fresh `fzf --filter` process for each launcher query when fuzzy fallback is needed. The GUI owns the input field and result list; `fzf` receives structured candidates plus the current query and returns ranked rows. This is simpler and easier to validate than a persistent background process because there is no long-lived mutable subprocess state and no protocol for live query mutation.

The downside is process startup overhead. That is acceptable for the current CLI and backend slice, but the architecture keeps `FzfBackend` isolated so a later Wayland UI can move querying to a worker thread or replace the internals with a persistent process.

## Candidate Transport

Candidates are structured internally and serialized as tab-separated rows:

```text
ID<TAB>PRIMARY<TAB>SECONDARY<TAB>SEARCHABLE
```

`--with-nth 2..` hides the ID from display, while `--nth 2..` keeps matching focused on user-visible/searchable fields. The returned row still includes the ID, so duplicate visible labels remain safe. Tabs, newlines, and NUL bytes are rejected before sending data to `fzf`.

Mode-specific search scope matters:

- `run` and `dmenu` currently send their primary labels to `fzf`
- `drun` currently sends only desktop application `Name` to `fzf`
- `drun` keeps `GenericName` available for exact/prefix/substring matching before fuzzy fallback
- `drun` does not send desktop `Comment` to `fzf`

This keeps fuzzy ranking useful without allowing unrelated comment text to dominate results.

## Pre-filter Stage

Before spawning `fzf`, the backend may return direct matches without fuzzy fallback:

- exact match
- prefix match
- substring match

At present this is used to prefer stronger `Name` and `GenericName` hits ahead of fuzzy matching. This is part of the backend contract, not a GUI-only presentation trick, so CLI and GUI remain consistent.

## Cancellation

The GUI path now supports explicit cancellation of superseded queries:

- starting a new query cancels the previous in-flight query
- clearing the query cancels any in-flight query
- closing the window cancels any in-flight query

Cancellation is implemented in Rust code and propagates down to the managed `fzf` subprocess so stale work is not only ignored, but actively stopped.

## I/O and Timeouts

The backend pipes stdin, stdout, and stderr explicitly. stdout and stderr are drained on worker threads so the child cannot block on full pipes. The configured timeout applies to each backend query.

## Failure Handling

The backend reports typed errors for missing binaries, spawn failures, missing pipes, invalid candidate fields, duplicate IDs, invalid output, unknown IDs, non-UTF-8 output, failed exit statuses, worker panics, explicit cancellation, and timeouts. `fzf` exit code `1` is treated as a valid “no matches” result.

## Future Persistent Mode

A persistent process may reduce latency for the graphical launcher, but it requires a carefully documented control protocol for query updates, candidate refreshes, cancellation, and stale result suppression. Until that protocol is implemented and benchmarked, restart-per-query is the production baseline.

## Future Refinement

If ranking still feels too permissive in `drun`, the next planned tightening step is:

- exact/prefix/substring-only for `drun`
- no fuzzy fallback when the query is already a strong desktop-name hit
