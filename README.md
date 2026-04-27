# Log Archive Orchestrator

[![CI](https://github.com/DarrylGamroth/iox2-log-archive-orchestrator/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/DarrylGamroth/iox2-log-archive-orchestrator/actions/workflows/ci.yml)
[![Codecov](https://codecov.io/gh/DarrylGamroth/iox2-log-archive-orchestrator/branch/main/graph/badge.svg)](https://codecov.io/gh/DarrylGamroth/iox2-log-archive-orchestrator)

`iceoryx2-userland-log-archive-orchestrator` is a userland control-plane daemon for managing many `iox2-log-recorder` workers.

## Model
- Control plane is versioned request/response over an iceoryx2 request-response service.
- Worker model is process-per-recorder (`iox2-log-recorder` subprocesses).
- Desired state is a durable TOML file.
- Runtime reconcile is periodic and command-triggered.

## Operating Pattern
1. Start `iox2-log-orchestrator serve` as the long-running daemon.
2. Send admin commands (`enable`, `disable`, `pause`, `resume`, `start`, `stop`, `status`, `list`, `reconcile`) with `iox2-log-orchestrator` CLI.
3. The daemon persists desired intent in `state.toml` (`--state-path`).
4. The daemon reconciles desired state to runtime by spawning/stopping `iox2-log-recorder` workers.
5. Worker liveness and stop operations are mediated through `iox2-log-control`.

Control flow:
- `iox2-log-orchestrator` CLI (client) -> orchestrator control service -> orchestrator daemon
- orchestrator daemon -> `iox2-log-recorder` worker processes
- orchestrator daemon -> `iox2-log-control` (status/stop against workers)

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
cargo test --all-targets --no-fail-fast
```

Release-gate checks:

```bash
cargo fmt --all --check
cargo check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets --no-fail-fast
```

Local coverage uses `cargo-llvm-cov`:

```bash
cargo install cargo-llvm-cov
./scripts/coverage.sh
```

The default coverage command writes `target/llvm-cov/lcov.info`. For an HTML
report, run `./scripts/coverage.sh --html`.

CI runs the same formatting, check, clippy, test, and coverage commands on
pushes and pull requests to `main`.

## Dependencies

Runtime expects these binaries to be available unless overridden:

```text
iox2-log-recorder
iox2-log-control
```

Both are provided by the sibling `iox2-log-archive` repository. The
orchestrator treats them as external process/control contracts and does not
link the archive core into the daemon.

## Quickstart

Terminal 1:

```bash
STATE=/tmp/iox2-log-orchestrator/state.toml
CONTROL_SERVICE=iox2/log/archive/orchestrator/control

cargo run --bin iox2-log-orchestrator -- \
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

cargo run --bin iox2-log-orchestrator -- \
  --format JSON \
  --state-path "$STATE" \
  --control-service "$CONTROL_SERVICE" \
  enable \
  --service "$SERVICE" \
  --instance default \
  --storage-path /tmp/iox2-archive/storage \
  --metadata-log-path /tmp/iox2-archive/metadata

cargo run --bin iox2-log-orchestrator -- \
  --format JSON \
  --state-path "$STATE" \
  --control-service "$CONTROL_SERVICE" \
  status --service "$SERVICE"

cargo run --bin iox2-log-orchestrator -- \
  --format JSON \
  --state-path "$STATE" \
  --control-service "$CONTROL_SERVICE" \
  pause --service "$SERVICE"

cargo run --bin iox2-log-orchestrator -- \
  --format JSON \
  --state-path "$STATE" \
  --control-service "$CONTROL_SERVICE" \
  resume --service "$SERVICE"
```

Shutdown:

```bash
cargo run --bin iox2-log-orchestrator -- \
  --format JSON \
  --state-path "$STATE" \
  --control-service "$CONTROL_SERVICE" \
  shutdown
```

## State Schema
Default file path:
- `$HOME/.config/iox2/log-orchestrator/state.toml`

Example:

```toml
version = 1

[services."My/Camera/Service"]
enabled = true
paused = false
instance = "default"
generation = 1
storage_path = "/var/lib/iox2-log-archive/storage/MyCamera"
metadata_log_path = "/var/lib/iox2-log-archive/metadata/MyCamera"
profile = "throughput"
mode = "async"
cycle_time_ms = 10
flush_interval_ms = 100
async_io_backend = "io-uring-preferred"
io_uring_queue_depth = 256
io_submit_batch_max = 256
io_cqe_batch_max = 512
io_uring_register_files = true
checksum_mode = "crc32c"
out_of_space_policy = "fail-writer"
metadata_log_roll_bytes = 67108864
metadata_log_max_bytes = 1073741824
```

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
- `services.<service>.max_disk_bytes`
- `services.<service>.async_io_backend`
- `services.<service>.io_uring_queue_depth`
- `services.<service>.io_submit_batch_max`
- `services.<service>.io_cqe_batch_max`
- `services.<service>.io_uring_register_files`
- `services.<service>.checksum_mode`
- `services.<service>.out_of_space_policy`
- `services.<service>.metadata_log_roll_bytes`
- `services.<service>.metadata_log_max_bytes`

All production tuning fields are optional. If omitted, `iox2-log-recorder`
uses the defaults selected by `profile`.

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

## Operational Runbook

- Start one daemon per host with a durable `--state-path`.
- Use `enable` to persist desired recorder configuration and start/reconcile immediately.
- Use `pause` for temporary runtime gating while keeping desired intent.
- Use `disable` to clear desired intent and request worker stop.
- Use `stop` only for immediate runtime stop; the next reconcile may restart the worker if intent is still enabled and not paused.
- Use `daemon-status`, `list`, and `status --service <name>` for health checks.
- If a service becomes degraded, inspect `last_error`, fix the external recorder/control/storage issue, then use `resume`, `start`, or `reconcile` as appropriate.

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
- Pub/sub recorder workers are the active supported transport. The retired core
  `Log` messaging pattern is intentionally not orchestrated.
- Deferred decisions are documented in `docs/log-archive-orchestrator-plan.md`.

## Traceability
- `docs/log-archive-orchestrator-traceability.md`
