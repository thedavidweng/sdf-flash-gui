# ADR 0011: The web demo reuses the desktop GUI behind a simulated backend

- **Status**: Accepted
- **Date**: 2026-10-04
- **Supersedes**: none

## Context

The project site needs an interactive demo. A separately built mock UI would
drift from the real app on every UI change. egui runs in the browser through
eframe's `WebRunner`, but the browser build (`wasm32-unknown-unknown`) has no
threads, no processes, no filesystem, no synchronous file dialogs, and no
drive hardware. `std::thread::spawn`, `Instant::now`, and `SystemTime::now`
panic there; `std::process` and `std::fs` return errors.

## Decision

- The demo is the desktop `App` with the same views, ops, workers, and start
  gate. `gui::create_web_demo` builds it; `web/` is a thin `cdylib` that only
  mounts it on a canvas. Native-only entry code (`gui::run`, the window
  icon, `rfd`) is `cfg`-gated off wasm32.
- The backend is simulated at the existing `ProcessRunner` seam:
  [`DemoRunner`](../../src/gui/demo.rs) answers `-l` and `--info` with canned
  output, refuses everything else, and requests a repaint.
- `workers::run_job` spawns a thread on native and runs the job inline on
  wasm32. With `DemoRunner` every job finishes immediately, so the UI thread
  is never blocked for long.
- `Host::WebDemo` is the only demo switch in shared code. Views ask
  `ops::system_access` before enabling file pickers, auto-detect, SDF
  parsing, and quit. The start gate skips host path validation and, once
  every other rule passes, blocks with `StartBlock::WebDemo`, so Start never
  enables and no read, write, or recover can begin.
- Settings and About are embedded egui windows on the web (no OS windows), so
  they get an in-window Close button whenever egui reports
  `ViewportClass::EmbeddedWindow`.

## Consequences

- UI changes reach the demo with no extra work; `scripts/build-site.sh` and
  the Pages workflow rebuild it from `main`.
- New code paths that touch threads, time, files, or processes must stay off
  the demo's reachable paths or be gated by `ops::system_access`. CI keeps
  the wasm32 build compiling (`clippy-wasm` job).
- `DemoRunner` fixtures must stay parseable by the real `--info` and `-l`
  parsers; `gui::demo` tests run the full list → probe → start gate pipeline
  to catch drift.
