# Log Archive Orchestrator

`iceoryx2-userland-log-archive-orchestrator` is a userland control-plane daemon for managing many `iox2-log-recorder` workers.

## Model
- Control plane is versioned request/response over an iceoryx2 request-response service.
- Worker model is process-per-recorder (`iox2-log-recorder` subprocesses).
- Desired state is a durable TOML file.
- Runtime reconcile is periodic and command-triggered.

## Commands
Binary: `iox2-log-orchestrator`

- `serve` (daemon mode)
- `enable`
- `disable`
- `pause`
- `resume`
- `start`
- `stop`
- `status`
- `list`
- `reconcile`
- `daemon-status`
- `shutdown`

## Build

```bash
cargo build
```

## Quickstart

Terminal 1:

```bash
STATE=/tmp/iox2-log-orchestrator/state.toml
CONTROL_SERVICE=iox2/log/archive/orchestrator/control

cargo run -- \
  --format JSON \
  --state-path "$STATE" \
  --control-service "$CONTROL_SERVICE" \
  serve
```

Terminal 2:

```bash
STATE=/tmp/iox2-log-orchestrator/state.toml
CONTROL_SERVICE=iox2/log/archive/orchestrator/control
SERVICE=My/Camera/Service

cargo run -- \
  --format JSON \
  --state-path "$STATE" \
  --control-service "$CONTROL_SERVICE" \
  enable \
  --service "$SERVICE" \
  --instance default \
  --storage-path /tmp/iox2-archive/storage \
  --metadata-log-path /tmp/iox2-archive/metadata

cargo run -- \
  --format JSON \
  --state-path "$STATE" \
  --control-service "$CONTROL_SERVICE" \
  status --service "$SERVICE"

cargo run -- \
  --format JSON \
  --state-path "$STATE" \
  --control-service "$CONTROL_SERVICE" \
  pause --service "$SERVICE"

cargo run -- \
  --format JSON \
  --state-path "$STATE" \
  --control-service "$CONTROL_SERVICE" \
  resume --service "$SERVICE"
```

Shutdown:

```bash
cargo run -- \
  --format JSON \
  --state-path "$STATE" \
  --control-service "$CONTROL_SERVICE" \
  shutdown
```

## State Schema
Default file path:
- `$HOME/.config/iox2/log-orchestrator/state.toml`

Key fields:
- `version`
- `services.<service>.enabled`
- `services.<service>.paused`
- `services.<service>.instance`
- `services.<service>.generation`
- `services.<service>.storage_path`
- `services.<service>.metadata_log_path`
- `services.<service>.profile`
- `services.<service>.mode`
- `services.<service>.cycle_time_ms`
- `services.<service>.flush_interval_ms`

## Lifecycle Semantics
- `enable`: set intent enabled with service configuration and reconcile immediately.
- `disable`: clear intent enabled and reconcile immediately.
- `pause`: keep intent but gate runtime and reconcile immediately.
- `resume`: remove gate and reconcile immediately.
- `start`: force `enabled=true` and `paused=false`, then reconcile immediately.
- `stop`: immediate runtime stop request without clearing desired intent.

## Health and Restart
- Status includes:
- identity (`service`, `instance`, `generation`)
- heartbeat age
- last error
- last transition reason
- restart attempts
- degraded flag
- next retry delay
- Restart policy is bounded exponential backoff with jitter and degraded transition on retry-window exhaustion.

## Configuration Precedence
`CLI > env > config file > defaults`

Supported env variables:
- `IOX2_LOG_ORCH_STATE_PATH`
- `IOX2_LOG_ORCH_CONTROL_SERVICE`
- `IOX2_LOG_ORCH_RECORDER_BIN`
- `IOX2_LOG_ORCH_CONTROL_BIN`
- `IOX2_LOG_ORCH_CONTROL_TIMEOUT_MS`
- `IOX2_LOG_ORCH_RECONCILE_INTERVAL_MS`
- `IOX2_LOG_ORCH_BACKOFF_INITIAL_MS`
- `IOX2_LOG_ORCH_BACKOFF_FACTOR`
- `IOX2_LOG_ORCH_BACKOFF_MAX_MS`
- `IOX2_LOG_ORCH_BACKOFF_JITTER_PERCENT`
- `IOX2_LOG_ORCH_BACKOFF_MAX_WINDOW_MS`

## Notes
- V1 remains single-host.
- Pattern-specific orchestration beyond pub/sub recorder workers is deferred.
- Deferred decisions are documented in `docs/log-archive-orchestrator-plan.md`.

## Traceability
- `docs/log-archive-orchestrator-traceability.md`
