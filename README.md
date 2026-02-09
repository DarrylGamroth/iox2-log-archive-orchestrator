# Log Archive Orchestrator (Incubation)

`iceoryx2-userland-log-archive-orchestrator` is a thin userland overlay that manages many `iox2-log-recorder` workers.

Status: V1 standalone extraction from in-tree incubation.

## What It Does
- Persists desired stream configuration in a versioned TOML state file.
- Idempotent `enable` / `disable` operations per service.
- Uses `iox2-log-control` for live liveness/status/stop behavior.
- `reconcile` starts missing enabled workers and stops disabled running workers.

## Commands
Binary: `iox2-log-orchestrator`

- `enable`
- `disable`
- `status`
- `list`
- `reconcile`

## Build

```bash
cargo build
```

## Quickstart

```bash
STATE=/tmp/iox2-log-orchestrator/state.toml
SERVICE=My/Camera/Service

cargo run -- \
  --format JSON \
  --state-path "$STATE" \
  enable \
  --service "$SERVICE" \
  --storage-path /tmp/iox2-archive/storage \
  --metadata-log-path /tmp/iox2-archive/metadata

cargo run -- \
  --format JSON --state-path "$STATE" status --service "$SERVICE"

cargo run -- \
  --format JSON --state-path "$STATE" reconcile

cargo run -- \
  --format JSON --state-path "$STATE" disable --service "$SERVICE"
```

## State File
Default path:
- `$HOME/.config/iox2/log-orchestrator/state.toml`

Schema highlights:
- `version`
- `services.<service_name>.enabled`
- `services.<service_name>.storage_path`
- `services.<service_name>.metadata_log_path`
- `services.<service_name>.profile`
- `services.<service_name>.mode`
- `services.<service_name>.cycle_time_ms`
- `services.<service_name>.flush_interval_ms`

## Notes
- V1 is single-host and process-based.
- Liveness is intentionally derived from recorder control responses, not pid-only checks.
- Pattern-specific orchestration beyond pub/sub recorder workers is deferred.

## Operational Runbook
### Recommended Runtime Model
- Run orchestrator as a one-shot control CLI under an external supervisor or service manager.
- Use `reconcile` on startup/restart to converge desired state to live recorder workers.

### Restart Semantics
- Desired state survives restart in `state.toml`.
- After orchestrator restart, no implicit worker assumptions are made.
- Operator or supervisor runs `reconcile` to:
- start enabled-but-missing services,
- stop disabled-but-live services,
- leave enabled-and-live services untouched.

### Failure Semantics
- `InvalidInput` failures return exit code `2` (invalid command/arguments/state request).
- `NotAvailable` failures return exit code `3` (external dependency unavailable).
- `Internal` failures return exit code `1` (unexpected/runtime execution failure).

Common operational cases:
- `iox2-log-control` unavailable:
- `status` reports service not available; `reconcile` treats enabled services as missing and attempts spawn.
- Recorder spawn failure:
- command fails with `Internal`; no desired-state rollback is performed.
- Malformed/unsupported state file:
- load fails; command exits with explicit serialized error.

### Idempotency Rules
- `enable` with identical service spec is a no-op update (`changed=false`).
- `disable` on an already-disabled service is a no-op update (`changed=false`).
- `reconcile` is safe to run repeatedly.

## Traceability
- `docs/log-archive-orchestrator-traceability.md`
