# Architecture

This document defines the architecture for `vega`, a rofi-like launcher with a managed `fzf` backend and a custom Rust GUI.

This is a showcase-grade product. The system must be designed for:

- performance
- correctness
- extensibility
- maintainability
- public release quality

______________________________________________________________________

## 1. Product Overview

The goal is to build a launcher that combines:

- rofi-like UX and theming
- controlled fuzzy search and ranking
- Wayland-native performance
- clean and maintainable architecture

Current prototype name:

- `vega`

Expected usage:

- `vega -show drun`

______________________________________________________________________

## 2. Core Design Decision

The current search backend is an installed system `fzf` binary invoked by Rust code through `std::process::Command`.

Two acceptable long-term approaches remain:

### Option A (Future Preferred)

Implement an internal fzf-inspired engine:

- no subprocess overhead
- full control over scoring
- easier integration with UI
- better explainability

### Option B (Current Baseline)

Integrate `fzf` as a backend:

- controlled subprocess execution
- structured input/output
- tightly integrated

### Explicit Constraint

Do NOT build fragile pipelines like:

rofi | fzf | rofi

The system must behave like a single coherent application. Search policy belongs to Rust code; `fzf` is an implementation detail behind that policy.

______________________________________________________________________

## 3. System Overview

CLI / GUI Entry
↓
Application Controller
↓
Search Coordinator
↓
Managed `fzf` Backend
↓
Modes / Data Providers
↓
Platform Integration

Cross-cutting:

- theme engine
- config system
- cache layer
- diagnostics
- matching policy

______________________________________________________________________

## 4. Core Subsystems

### 4.1 UI Layer

Responsibilities:

- native window creation through the chosen Rust windowing stack
- rendering input and results
- keyboard handling
- focus management
- theming application

Constraints:

- no search logic
- no provider logic
- must remain responsive

Current implementation:

- `src/gui.rs` with `eframe`/`egui`
- refined header row with fixed-width mode badge and larger input typography
- background query worker threads
- generation-based stale result suppression
- explicit cancellation of superseded in-flight queries

______________________________________________________________________

### 4.2 Application Controller

Responsibilities:

- manage query state
- manage selection
- coordinate filtering
- switch modes
- handle submit and cancel

Design:

- explicit
- predictable
- not a god object

Current implementation:

- split between `src/main.rs` and `src/gui.rs`
- GUI path owns selection state and query lifecycle
- CLI path owns one-shot query execution

______________________________________________________________________

### 4.3 Search Coordinator and Backend

Responsibilities:

- apply match policy before fuzzy fallback
- serialize structured candidates safely
- invoke `fzf` as a subprocess when needed
- cancel superseded subprocess-backed queries
- parse ranked output
- enforce timeout and error handling

Current matching stages:

- exact match
- prefix match
- substring match
- subsequence fuzzy match through `fzf`

Current policy:

- `run` and `dmenu` use fuzzy backend matching on primary labels
- `drun` fuzzy matching uses desktop `Name`
- `drun` `GenericName` is available for stronger direct matches before fuzzy fallback
- desktop `Comment` is excluded from `drun` search scope

Future policy refinement under consideration:

- exact/prefix/substring-only for `drun`
- no fuzzy fallback when the query is already a strong name hit

Debug visibility must include:

- configured `fzf` binary name
- resolved executable path
- candidate/result counts
- elapsed query time

______________________________________________________________________

### 4.4 Modes / Data Providers

Modes provide candidates via a common interface.

Examples:

- drun (desktop apps)
- run (executables)
- dmenu (stdin)
- window (window switcher)
- keys (keybinding/help)

Each mode defines:

- how candidates are loaded
- what fields are searchable
- what happens on execution
- refresh behavior

Current implementations:

- `dmenu`: stdin lines, label search, print selected label
- `run`: executables from `PATH`, direct `Command` execution
- `drun`: `.desktop` entries, conservative `Exec` parsing, direct `Command` execution

______________________________________________________________________

### 4.5 Theme Engine

Responsibilities:

- parse theme config
- validate styles
- apply layout and visuals

Supports:

- window styling
- input styling
- list styling
- selected/active states
- spacing and layout
- fonts and icons

Must be:

- deterministic
- maintainable
- validated

Current state:

- hard-coded `egui` styling in `src/gui.rs`
- no external theme config yet

______________________________________________________________________

### 4.6 Config System

Responsibilities:

- load config files
- validate schema
- apply defaults

Preferred format:

- TOML

Config includes:

- modes
- theme
- ranking options
- keybindings
- placement

Current state:

- not implemented
- debug behavior and backend flags are CLI-driven

______________________________________________________________________

### 4.7 Cache Layer

Used to improve startup and runtime performance.

Cache examples:

- desktop entries
- executable list
- icon metadata

Requirements:

- versioned
- safe invalidation
- corruption fallback

Current state:

- not implemented

______________________________________________________________________

## 5. Current Module Layout

Current code is organized as:

- `src/main.rs`: CLI parsing, mode dispatch, one-shot query path
- `src/gui.rs`: current GUI window, input handling, result list, worker-thread integration
- `src/fzf.rs`: subprocess lifecycle, candidate transport, pre-filter stage, timeout/error handling
- `src/modes.rs`: `dmenu`, `run`, and `drun` providers plus execution logic
- `src/candidate.rs`: structured candidate model and execution actions

This is intentionally smaller than the eventual target layout and should evolve into dedicated `ui/`, `search/`, `config/`, and diagnostics modules as the project grows.

______________________________________________________________________

## 6. Architectural Constraints

- No shell pipelines for core launcher behavior.
- Execution must use structured `Command` invocation, not a shell.
- Match scope must be mode-specific and explicit in code.
- GUI and CLI search behavior must share the same backend policy.
- Search diagnostics must make backend selection observable in debug mode.
- Future refactors must preserve duplicate-label safety through structured candidate IDs.

______________________________________________________________________

## 7. Near-Term Plan

1. Move GUI code from `src/gui.rs` into a fuller `src/ui/` tree.
1. Add integration tests with a fake `fzf` binary.
1. Benchmark restart-per-query before attempting persistent mode.
1. Re-evaluate `drun` strictness and potentially stop fuzzy fallback once strong name hits exist.
1. Add config loading and theme/config separation as the GUI grows.

______________________________________________________________________

### 4.8 Diagnostics

Includes:

- logging
- timing
- debug modes
- ranking explainability

______________________________________________________________________

## 5. Candidate Model

Candidates must be structured, not raw strings.

Conceptually:

Candidate:

- id
- primary_text
- secondary_text
- searchable_fields
- icon
- metadata
- flags
- action

This allows:

- better ranking
- richer UI
- flexible modes

______________________________________________________________________

## 6. Search Design

### Matching Types

- exact
- prefix
- substring
- fuzzy subsequence

### Ranking Goals

Boost:

- prefix matches
- boundary matches
- contiguous matches
- shorter results

Penalty:

- gaps
- late matches
- long strings

### Determinism

Tie-breaking must be stable.

______________________________________________________________________

## 7. Mode Interface

Each mode should expose:

- load_candidates
- filter(query)
- execute(candidate)
- refresh()

Optional:

- async loading
- caching

______________________________________________________________________

## 8. UI Flow

User types\
→ controller updates query\
→ search engine ranks candidates\
→ UI renders results\
→ user selects\
→ action executes

______________________________________________________________________

## 9. Wayland Integration

Requirements:

- native Wayland client
- low startup cost
- proper focus handling

Avoid heavy frameworks unless justified.

______________________________________________________________________

## 10. Performance Targets

- fast startup
- near-instant filtering
- smooth typing experience
- no UI freezes

______________________________________________________________________

## 11. Error Handling

Must:

- fail gracefully
- log clearly
- isolate bad data
- avoid crashing UI

______________________________________________________________________

## 12. Security

Must:

- avoid shell injection
- safely execute commands
- correctly parse desktop entries

______________________________________________________________________

## 13. Testing Strategy

### Unit tests

- matching
- scoring
- parsing

### Integration tests

- modes
- execution
- config

### Performance tests

- ranking latency
- startup time

______________________________________________________________________

## 14. Repository Structure

project-root/
README.md
docs/
architecture.md
src/
app/
ui/
search/
modes/
config/
cache/
platform/
diagnostics/

______________________________________________________________________

## 15. Key Decisions

### Why fzf-style backend?

Because ranking quality defines the product.

### Why not naive fzf piping?

- poor integration
- fragile behavior
- not production quality

### Why structured architecture?

To avoid:

- tight coupling
- unmaintainable growth
- hidden complexity

______________________________________________________________________

## 16. Risks

- ranking quality tuning
- desktop entry correctness
- theme complexity
- Wayland edge cases

______________________________________________________________________

## 17. Development Order

1. search engine
1. benchmarks
1. controller
1. UI shell
1. modes
1. theme
1. performance tuning

______________________________________________________________________

## 18. Guiding Principles

- clarity over cleverness
- explicit over implicit
- performance over abstraction
- design over patching

## 19. fzf Backend Integration (Mandatory)

The application must use **fzf as the backend search engine**, not a reimplementation.

This is a deliberate product decision.

### Rationale

- fzf already provides best-in-class fuzzy matching
- battle-tested scoring behavior
- predictable ranking users trust
- avoids reinventing complex ranking logic

### Constraint

fzf must be integrated as a **core backend component**, not used as a loose CLI pipe.

The following patterns are NOT acceptable:

- `rofi | fzf | rofi`
- ad-hoc shell pipelines
- unstructured stdin/stdout chaining

The integration must be:

- controlled
- structured
- efficient
- observable

______________________________________________________________________

## 19.1 Integration Strategy

fzf will run as a managed subprocess.

The application must:

- spawn fzf as a child process
- communicate via stdin/stdout
- maintain lifecycle control
- handle cancellation and restart cleanly

### Data Flow

Application → sends candidate list → fzf stdin\
fzf → performs filtering → outputs ranked results → stdout\
Application → parses → renders UI

______________________________________________________________________

## 19.2 Interaction Model

Two possible models:

### Model A (preferred)

- application sends full candidate list
- fzf runs interactively
- application queries results continuously or per input change

### Model B

- application sends query + candidates
- fzf runs per query (stateless execution)

Trade-offs must be documented.

______________________________________________________________________

## 19.3 Performance Considerations

Risks:

- subprocess overhead
- large input streaming cost
- synchronization latency

Mitigations:

- reuse fzf process when possible
- batch input efficiently
- limit candidate set early (mode-level filtering)
- avoid full reload on every keystroke if possible

______________________________________________________________________

## 19.4 Explainability Limitation

Because fzf is external:

- scoring internals are opaque
- full explainability is not available

Mitigation:

- provide debug output of:
  - input candidates
  - filtered results
  - ordering
- optionally expose fzf flags used

______________________________________________________________________

## 19.5 Configuration of fzf

Expose configurable options:

- matching mode
- case sensitivity
- scoring behavior flags
- sorting behavior

These must be mapped cleanly into application config.

______________________________________________________________________

## 19.6 Failure Handling

Must handle:

- fzf not installed
- subprocess failure
- broken pipe
- timeout or hang

Fallback behavior:

- graceful error message
- optional degraded mode (no search)
- clear logs

______________________________________________________________________

## 19.7 Future Option

Architecture should allow future replacement with:

- internal matching engine
- alternative fuzzy engine

This must be possible without rewriting UI or modes.

______________________________________________________________________

## 19.8 Design Constraint Summary

- fzf is mandatory backend
- integration must be clean and controlled
- no shell hacks
- no fragile piping
- lifecycle must be owned by the application

## 20. IPC Protocol Design (fzf Communication)

fzf is integrated as a subprocess. Communication must be structured and predictable.

### 20.1 Communication Channels

- stdin → send candidates
- stdout → receive filtered results
- stderr → logging/debugging

### 20.2 Data Format

Candidates must be serialized into a stable format.

Recommended:

- plain text lines for fzf input
- structured encoding for internal mapping

Example:

ID<TAB>PRIMARY_TEXT<TAB>SECONDARY_TEXT

Only visible parts should be shown to fzf unless extended display is needed.

### 20.3 ID Mapping

fzf returns selected lines as text.

Application must:

- map selected line back to internal candidate ID
- avoid ambiguity (no duplicate display strings without IDs)

### 20.4 Query Handling

Two strategies:

#### Streaming mode (preferred if feasible)

- keep fzf running
- update input incrementally

#### Restart mode

- restart fzf per query
- simpler but slower

Decision must be documented and benchmarked.

### 20.5 Synchronization

Ensure:

- no race conditions between input updates and output reads
- proper flushing of stdin
- non-blocking reads from stdout

### 20.6 Cancellation

On query change:

- terminate or reset fzf process
- discard stale output
- ensure UI consistency

______________________________________________________________________

## 21. Process Lifecycle Management

The application must fully control the fzf process.

### 21.1 Lifecycle

- spawn on demand
- reuse if possible
- terminate on exit
- restart on failure

### 21.2 Failure Modes

Handle:

- fzf not found
- process crash
- broken pipe
- hung process

### 21.3 Timeouts

Implement:

- startup timeout
- response timeout

Kill process if unresponsive.

### 21.4 Resource Control

Ensure:

- no zombie processes
- proper cleanup on exit
- minimal process churn

______________________________________________________________________

## 22. Mode-Level Optimization

fzf should not receive unbounded datasets.

### 22.1 Pre-filtering

Modes should:

- reduce candidate size early
- apply domain-specific filtering

Examples:

- drun → ignore hidden entries
- run → deduplicate PATH
- window → filter invisible windows

### 22.2 Chunking (if needed)

For large datasets:

- chunk input
- lazy load candidates

### 22.3 Caching

Modes should cache:

- expensive scans
- parsed data

______________________________________________________________________

## 23. UI and fzf Synchronization

UI must remain responsive regardless of fzf state.

### 23.1 Input Handling

- user input must not block
- debounce query updates if needed

### 23.2 Rendering

- render last known results immediately
- update asynchronously when new results arrive

### 23.3 State Consistency

Avoid:

- flickering
- stale results
- inconsistent selection index

______________________________________________________________________

## 24. Performance Strategy

### 24.1 Targets

- sub-50ms perceived latency per keystroke
- minimal startup delay
- smooth UI updates

### 24.2 Bottlenecks

- process spawn cost
- large stdin writes
- stdout parsing

### 24.3 Mitigation

- reuse fzf process
- minimize candidate size
- avoid full refresh per keystroke
- use efficient buffering

______________________________________________________________________

## 25. Logging and Debugging

Logging must be structured and useful.

### 25.1 Log Levels

- error
- warning
- info
- debug

### 25.2 Debug Modes

Provide:

- fzf command dump
- candidate input preview
- result output preview

### 25.3 Ranking Debug

Even with fzf, expose:

- input set
- output order
- query used

______________________________________________________________________

## 26. Configuration for fzf Backend

Expose fzf-related settings:

- matching behavior flags
- case sensitivity
- sorting options
- preview settings (optional)

Example config:

fzf:
case: smart
layout: default
extra_flags: []

Mapping must be documented clearly.

______________________________________________________________________

## 27. Extensibility Strategy

Even though fzf is used, architecture must allow future changes.

### 27.1 Replaceability

Search engine must be abstracted:

- interface-based
- minimal coupling with UI

### 27.2 Future Backends

Possible future options:

- internal engine
- alternative fuzzy libraries

### 27.3 Plugin Support (future)

Modes should eventually be pluggable.

______________________________________________________________________

## 28. Packaging and Distribution

### 28.1 Requirements

- fzf must be installed or bundled
- clear dependency documentation

### 28.2 Distribution Options

- Arch package (primary target)
- binary release
- optional static build

### 28.3 Runtime Checks

On startup:

- verify fzf availability
- fail fast with clear message

______________________________________________________________________

## 29. Release Readiness Checklist

Before public release:

- startup performance acceptable
- no crashes under normal use
- fzf integration stable
- all modes functional
- config validated
- logs clean
- documentation complete

______________________________________________________________________

## 30. Final Notes

This project intentionally trades:

- control over ranking internals

for:

- proven fuzzy matching behavior (fzf)

This is acceptable for a v1 showcase product, provided:

- integration is clean
- performance is acceptable
- architecture remains flexible

Future evolution must remain possible without rewriting the entire system.

## 31. Request to Codex: Production-grade fzf Process Wrapper

Codex: implement a **production-grade Rust wrapper around fzf** for this launcher architecture.

This is **not** a request to build the whole app here.\
This is a request to design and implement the **fzf backend integration layer** only.

Treat this as high-stakes product engineering for a showcase release.

### What you must produce

Create a clean, reviewable backend subsystem that:

- spawns and manages `fzf` as a subprocess
- streams candidates to `fzf`
- reads results from `fzf`
- handles cancellation, restart, and cleanup safely
- exposes a clean Rust interface to the rest of the application
- is documented like production code, not demo code

### Required output

You must deliver:

1. a Rust module structure for the fzf backend
1. the core process lifecycle design
1. the public interface/API for the wrapper
1. error types and failure handling strategy
1. buffering and I/O design
1. cancellation and restart behavior
1. timeout strategy
1. test strategy
1. benchmark considerations
1. documentation for all major design choices

### Implementation expectations

Design this like a principal engineer.

That means:

- no fragile shell hacks
- no ad hoc stringly typed interfaces
- no blocking UI assumptions
- no hidden global state
- no vague error handling
- no zombie process risk
- no hand-wavy lifecycle logic

### Architectural constraints

The wrapper must support:

- starting `fzf`
- feeding candidates through stdin
- reading output through stdout
- capturing stderr for debug logs
- killing or restarting the subprocess cleanly
- discarding stale results safely
- mapping displayed rows back to internal candidate IDs

### Candidate transport

Assume candidates are internally structured and must be serialized for `fzf`.

Support a stable line format such as:

ID<TAB>PRIMARY<TAB>SECONDARY

You must define:

- how IDs are encoded
- how the display text is generated
- how selected lines are mapped back to internal candidate records
- how duplicate visible labels are handled safely

### API requirement

Propose and implement a clean Rust-facing API, for example conceptually:

- `FzfBackend::start(...)`
- `FzfBackend::query(...)`
- `FzfBackend::update_candidates(...)`
- `FzfBackend::read_results(...)`
- `FzfBackend::shutdown(...)`

You may refine the exact API, but keep it small, explicit, and testable.

### Process model

You must explicitly evaluate and document both designs:

#### A. Persistent process

Keep `fzf` alive and reuse it across queries.

#### B. Restart-per-query process

Spawn a fresh `fzf` instance when needed.

You must recommend one approach for v1 and explain why.

### Failure handling

Handle at minimum:

- `fzf` binary not found
- spawn failure
- broken stdin pipe
- broken stdout pipe
- invalid output format
- timeout
- hung subprocess
- forced cancellation during active read/write
- partial write or partial read
- shutdown while work is in flight

### Concurrency and responsiveness

Assume the UI must stay responsive.

Your design must clearly explain:

- what runs synchronously
- what runs asynchronously
- how cancellation works
- how stale results are prevented from reaching the UI
- how process state is synchronized safely

### Logging and diagnostics

Include structured debug logging for:

- process start/stop
- command and flags used
- candidate counts
- query updates
- result counts
- stderr output
- timeout or crash conditions

### Documentation requirement

Document every major design decision.

For each important choice, explain:

- what you chose
- what alternatives you considered
- why this choice is appropriate here
- what downside it introduces

You must also create or update the following docs:

- `docs/fzf-backend.md`
- `docs/dev-notes.md`

### Testing requirement

Provide a test plan covering:

- normal process start/stop
- candidate streaming
- valid result parsing
- duplicate labels
- cancellation during active query
- restart after failure
- timeout handling
- cleanup on drop/shutdown

If full integration tests are hard, say so clearly and provide the best achievable structure.

### Benchmark requirement

Add benchmark notes for:

- process startup overhead
- candidate write throughput
- result read latency
- effect of candidate count on perceived responsiveness

### Naming note

If useful, you may also propose a **CLI-friendly product name** similar in spirit to `rofi`:

- short
- lowercase
- memorable
- easy to type

The name should work naturally in commands like:

- `name -show drun`
- `name -show run`

Do not let naming derail the backend work. It is optional and secondary.

### Final instruction

Do not build a toy wrapper.

Build something that could realistically sit inside a public, release-grade Linux launcher.
Document it thoroughly.
Reiterate important design choices in the docs so the architecture reads as deliberate and reviewable.
