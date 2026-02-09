# Log Archive Orchestrator Plan

## Status
- In progress
- Last updated: 2026-02-09
- Repository: `iox2-log-archive-orchestrator` (standalone extraction)
- Base compatibility target: `iceoryx2` upstream `main` / HEAD

## Objective
Deliver a thin overlay orchestrator for `log-archive` that manages many per-service recorder daemons while keeping `log-archive` core hot paths policy-free.

## Scope
### In Scope (V1)
- Persist desired recorder stream-set configuration.
- Reconcile desired state to running `iox2-log-recorder` workers.
- Stream lifecycle operations: `enable`, `disable`, `status`, `list`, `reconcile`.
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
- `orchestrator`: enable/disable/list/reconcile logic.
- `cli`: operator surface.

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

## Progress Tracker
- [x] Phase 0 complete
- [x] Phase 1 complete
- [x] Phase 2 complete
- [x] Phase 3 complete
- [x] Phase 4 complete
- [ ] Phase 5 complete

## Open Decisions
- `OD-001`: Process ownership model: orchestrator-daemon loop vs one-shot CLI + external supervisor.
- `OD-002`: Should state be single file or split (`desired.toml` + runtime watermark file)?
- `OD-003`: Worker restart policy defaults (immediate retry vs backoff).
- `OD-004`: Whether orchestrator control plane should be exposed as request/response service in V1.
