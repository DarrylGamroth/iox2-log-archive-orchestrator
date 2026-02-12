# Log Archive Orchestrator Plan

## Status
- In progress
- Last updated: 2026-02-12
- Repository: `iox2-log-archive-orchestrator` (standalone extraction)
- Base compatibility target: `iceoryx2` upstream `main` / HEAD

## Objective
Deliver a thin overlay orchestrator for `log-archive` that manages many per-service recorder daemons while keeping `log-archive` core hot paths policy-free.

## Scope
### In Scope (V1)
- Persist desired recorder stream-set configuration.
- Reconcile desired state to running `iox2-log-recorder` workers.
- Stream lifecycle operations: `enable`, `disable`, `pause`, `resume`, `start`, `stop`, `status`, `list`, `reconcile`.
- Leverage existing recorder control plane (`iox2-log-control` protocol) for liveness and stop.
- Deterministic, machine-readable CLI outputs and exit codes.

### Out of Scope (Deferred)
- Full metadata DB ownership and schema customization.
- Replay job scheduling and multi-tenant replay routing.
- FITS/data-product generation workflows.
- Pattern adapters beyond pub/sub ingest orchestration.
- Distributed orchestration (single-host orchestrator only in V1).

## Normative Language
The key words `MUST`, `SHOULD`, `MAY` are to be interpreted as described in RFC 2119 / RFC 8174 when capitalized.

## V1 Requirements
- `ORCH-001`: Orchestrator MUST remain userland-only and treat recorder/archive internals as external contracts.
- `ORCH-002`: Desired state MUST be persisted on disk and survive orchestrator restarts.
- `ORCH-003`: `enable(service)` MUST be idempotent.
- `ORCH-004`: `disable(service)` MUST be idempotent.
- `ORCH-005`: `reconcile` MUST only start missing enabled workers and MUST NOT duplicate running workers.
- `ORCH-006`: Liveness/status MUST be determined from recorder control protocol, not only local pid assumptions.
- `ORCH-007`: CLI outputs MUST be deterministic and serializable (`RON`/`JSON`/`YAML`).
- `ORCH-008`: Failure modes MUST return stable exit codes and explicit error payloads.
- `ORCH-009`: Worker process command construction MUST be deterministic from persisted config.
- `ORCH-010`: V1 MUST support pub/sub recorder workers; other pattern-specific orchestration MAY be added later.

## Architecture
- Standalone crate/binary project: `iceoryx2-userland-log-archive-orchestrator`.
- Components:
- `state`: desired-state persistence (TOML file).
- `control`: recorder control protocol client (`status`/`stop` probes).
- `worker`: deterministic `iox2-log-recorder` spawn contract.
- `daemon`: request/response control service server with reconcile loop.
- `orchestrator`: lifecycle/state/reconcile logic.
- `cli`: operator surface as daemon client plus `serve`.
- `config`: merged settings with precedence `CLI > env > file > defaults`.

### Runtime Pattern
1. `serve` is the long-running authority for desired-vs-actual convergence.
2. CLI invocations are request/response control clients to `serve`.
3. `serve` persists desired intent in `state.toml`.
4. `serve` reconciles by spawning/stopping `iox2-log-recorder` workers.
5. `serve` uses `iox2-log-control` for worker liveness/status/stop protocol.

## Implementation Phases
### Phase 0: Contract + Data Model
- Define persisted config schema and command semantics.
- Define error model and operation idempotency rules.
- Exit criteria:
- Requirements `ORCH-001..004` are reflected in doc and type model.

### Phase 1: Crate Scaffold + CLI Surface
- Add orchestrator crate to workspace.
- Implement CLI command skeleton (`enable`, `disable`, `status`, `list`, `reconcile`).
- Exit criteria:
- Binary builds and help surfaces all V1 commands.

### Phase 2: Desired-State Persistence
- Implement load/save with versioned schema.
- Implement idempotent enable/disable updates.
- Exit criteria:
- State roundtrip and idempotency tests pass.

### Phase 3: Control Plane Integration
- Implement recorder liveness/status probing through control protocol.
- Implement stop request path for disable.
- Exit criteria:
- Control probe tests pass (available/missing/protocol-mismatch cases).

### Phase 4: Reconcile Worker Management
- Deterministic worker command generation and spawn of missing enabled workers.
- Safe no-op for already running services.
- Exit criteria:
- Reconcile tests prove no duplicate launches for running services.

### Phase 5: Hardening + Docs
- Add operational runbook with restart and failure semantics.
- Add traceability table `ORCH-*` -> code/tests.
- Exit criteria:
- No unresolved V1 requirements.

### Phase 6: Control Plane Freeze + Execution Model
- Freeze control-plane transport as request/response with versioned command/response schema.
- Keep production execution model as process-per-recorder worker.
- Define canonical identity as `service + instance + generation` with monotonic per-service generation.
- Preserve single-file durable desired state with atomic write+rename+fsync.
- Lock lifecycle semantics:
- `enable`/`disable` for intent.
- `pause`/`resume` for temporary runtime gating.
- `start` as `enable + resume + immediate reconcile`.
- `stop` as immediate runtime stop while preserving desired intent.
- Run reconcile on interval and after every accepted command.
- Freeze restart policy as bounded exponential backoff with jitter and degraded-state transition.
- Freeze health contract fields in status: heartbeat freshness, last error, last transition reason, restart counters.
- Freeze config precedence: CLI > env > config file > profile defaults.
- Freeze compatibility policy: canonical commands only, no legacy aliases.
- Exit criteria:
- Phase 6 decisions are documented as normative constraints for implementation and traceability updates.

## Progress Tracker
- [x] Phase 0 complete
- [x] Phase 1 complete
- [x] Phase 2 complete
- [x] Phase 3 complete
- [x] Phase 4 complete
- [x] Phase 5 complete
- [x] Phase 6 complete

## Phase 6 Decision Freeze
- `DF-001`: Control API transport MUST be request/response service with versioned schema.
- `DF-002`: Worker model MUST be process-per-recorder for production; in-proc workers MAY be used only in tests.
- `DF-003`: Identity key MUST be `service + instance + generation` with monotonic per-service generation.
- `DF-004`: Desired-state store MUST be a single durable TOML file written with atomic replace and fsync.
- `DF-005`: Lifecycle semantics MUST use `enable/disable` for intent, `pause/resume` for runtime gating, `start` as `enable + resume + immediate reconcile`, and `stop` as immediate stop without clearing intent.
- `DF-006`: Reconcile MUST run both periodically and immediately after accepted commands.
- `DF-007`: Restart policy MUST use bounded exponential backoff with jitter, a max retry window, then degraded state.
- `DF-008`: Status MUST expose heartbeat freshness, last error, last transition reason, and restart counters.
- `DF-009`: Configuration precedence MUST be CLI > env > config file > profile defaults.
- `DF-010`: Compatibility policy MUST use canonical binaries/commands only and MUST NOT provide legacy aliases.

## Deferred Decisions
- `DD-001`: AuthN/AuthZ on orchestrator control plane.
- `DD-002`: Multi-host orchestration.
- `DD-003`: In-process worker mode for non-test production use.
- `DD-004`: Advanced admission control and quotas.

## Phase 5 Closure
- Operational runbook is documented in `README.md` (`Operational Runbook` section).
- Requirement traceability is documented in `docs/log-archive-orchestrator-traceability.md`.
- All `ORCH-001..ORCH-010` requirements are mapped to implementation and verification evidence.

## Phase 6 Closure
- Versioned control transport is implemented via iceoryx2 request/response envelopes (`CONTROL_API_VERSION=1`).
- Production execution uses process-per-recorder worker spawning (`ProcessRunner`); no in-proc production runner is enabled.
- Persisted service identity now includes `service + instance + generation` with monotonic generation bump on spec changes.
- Desired-state persistence is durable single-file TOML with atomic replace and fsync.
- Lifecycle commands include `pause`/`resume`/`start`/`stop` and trigger immediate reconcile behavior.
- Periodic reconcile loop and bounded retry/backoff with degraded-state transition are implemented in daemon runtime.
- Health/status now exposes heartbeat age, last error, last transition reason, restart attempts, and degraded state.
- Configuration precedence is implemented as `CLI > env > config file > defaults`.
