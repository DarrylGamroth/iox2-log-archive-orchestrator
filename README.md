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
