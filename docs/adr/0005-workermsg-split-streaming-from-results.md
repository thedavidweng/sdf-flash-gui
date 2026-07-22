# ADR 0005: Split WorkerMsg into streaming events and terminal results

- **Status**: Accepted
- **Date**: 2026-07-21
- **Supersedes**: none

## Context

`WorkerMsg` was a flat enum mixing two categories of messages:

- **Streaming events** (`Log`, `Progress`, `Status`) — incremental, many per
  operation, never produce user-attention notifications.
- **Terminal results** (`ProbeComplete`, `OperationComplete`, `DrivesListed`,
  `StopNeedsForceKill`) — exactly one per spawned task, may produce
  user-attention notifications.

The handler (`handle_worker_msg`) returned `Option<Attention>` for all
variants, but only terminal results ever returned `Some`. The distinction
between "this is a mid-operation update" and "this is the final outcome" was
implicit in the handler logic, not visible in the type.

## Decision

Split `WorkerMsg` into two nested enums:

```rust
pub enum WorkerMsg {
    Stream(StreamEvent),  // Log, Progress, Status
    Done(WorkerResult),   // ProbeComplete, OperationComplete, DrivesListed, StopNeedsForceKill
}
```

The handler dispatches on `WorkerMsg` then delegates:
- `handle_stream` — applies state mutations, returns `()`.
- `handle_result` — applies state mutations, returns `Option<Attention>`.

## Consequences

- The type system makes the streaming/result distinction explicit. A reader
  can see at a glance which messages are terminal.
- `handle_stream` cannot accidentally return an attention notification — the
  type is `()`.
- Send sites self-document intent: `tx.send(WorkerMsg::Stream(...))` vs
  `tx.send(WorkerMsg::Done(...))`.
- Exactly one `Done` per spawned task is now enforceable by convention and
  reviewable in diffs.
