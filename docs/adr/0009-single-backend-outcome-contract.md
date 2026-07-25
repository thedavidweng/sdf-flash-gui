# ADR 0009: One BackendOpError contract and one shared test fake

- **Status**: Accepted
- **Date**: 2026-07-25
- **Supersedes**: none

## Context

Three structurally identical outcome shapes crossed the orchestration→GUI
seam: `ProbeError { Failed, Cancelled, NeedsForceKill }`, `BackendOpError`
with the same three variants, and raw `String` from `execute_command_with` /
the streaming path. Each worker spawn function carried its own near-identical
match tail; the `NeedsForceKill` arm was byte-identical in all three.

The `ProcessRunner` seam also carried two module-private test fakes
(`OutcomeRunner` in orchestration tests, `MockRunner` in workers tests), each
re-implementing both trait methods and its own output builders.

## Decision

1. **`BackendOpError` is the single failure contract** for every cancellable
   backend operation: probe, list, streaming write/read, and execute.
   `ProbeError` is deleted; `probe_drive_with`, `run_list_backend_with`,
   `execute_command_with`, and the new `run_streaming_with` all return it.
   CLI-facing wrappers stringify at the edge (`probe_drive` adds its
   "cannot probe drive:" context).
2. **One failure adapter in workers.** `send_backend_error` maps
   `BackendOpError` to Worker messages (force-kill prompt / cancel notice /
   error log + failed result). The streaming and list spawns use it; the probe
   spawn matches the same enum for its probe-specific result shape.
3. **One shared fake.** `src/test_support.rs` (`cfg(test)`) provides
   `FakeRunner` for the `ProcessRunner` seam; both orchestration and workers
   test suites use it.

## Consequences

- A new backend operation kind inherits the error contract and the GUI
  failure mapping instead of adding a fourth enum and match tail.
- Behavior-preserving: display strings for cancel/force-kill/failure are
  unchanged.
- Fake behavior is defined once; divergence between the two test suites'
  doubles is no longer possible.
