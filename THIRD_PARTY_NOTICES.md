# Third-Party Notices

`vega` itself is licensed under the MIT License. See [LICENSE](./LICENSE).

This project depends on third-party Rust crates from crates.io. Based on the
current dependency metadata from `cargo metadata`, the dependency graph is
primarily licensed under permissive terms such as:

- `MIT OR Apache-2.0`
- `MIT`
- `Apache-2.0`

The current GUI stack also pulls in some additional license expressions through
transitive dependencies, including terms such as:

- `Unicode-3.0`
- `BSD-2-Clause`
- `BSD-3-Clause`
- `ISC`
- `Zlib`
- `BSL-1.0`
- `CC0-1.0`
- `OFL-1.1`
- `Ubuntu-font-1.0`

Notable direct dependency:

- `eframe 0.34.1` — `MIT OR Apache-2.0`
  - upstream: <https://github.com/emilk/egui/tree/main/crates/eframe>

## Important Distribution Note

If you distribute source archives or binaries of `vega`, you should preserve
any third-party notices required by the dependencies you ship, especially for:

- bundled fonts or font data
- Unicode data
- any crate whose license requires attribution text to travel with redistributions

This file is a summary, not a full legal bill of materials. Before public
release, generate and review a complete dependency license inventory from the
exact lockfile you ship.

One simple audit command used during development:

```bash
jq -r '.packages[] | select(.source != null) | .license // "UNKNOWN"' <(cargo metadata --format-version 1) | sort | uniq -c | sort -nr
```
