# Log Archive Orchestrator Traceability

Last updated: 2026-04-27

This matrix maps baseline `ORCH-*` requirements and Phase 6 freeze decisions `DF-*` to implementation and verification evidence.

## ORCH Requirement Matrix

| Requirement | Statement | Implementation | Verification | Status |
| --- | --- | --- | --- | --- |
| `ORCH-001` | Orchestrator MUST remain userland-only and treat recorder/archive internals as external contracts. | `src/control.rs`, `src/worker.rs`, `src/daemon.rs` (external binary contracts via `iox2-log-control` / `iox2-log-recorder`) | Architecture/dependency review (`Cargo.toml` has no direct core runtime dependency) | Covered |
| `ORCH-002` | Desired state MUST be persisted on disk and survive orchestrator restarts. | `src/state.rs` (`load_or_default`, durable `save` with atomic replace + fsync) | `state::tests::save_and_load_roundtrip`, `desired_state_roundtrip_preserves_service_spec` | Covered |
| `ORCH-003` | `enable(service)` MUST be idempotent. | `src/daemon.rs` (`enable`, spec comparison and stable `changed` semantics) | `lifecycle_commands_are_idempotent_and_reconciled` | Covered |
| `ORCH-004` | `disable(service)` MUST be idempotent. | `src/daemon.rs` (`disable`, enabled transition only) | `lifecycle_commands_are_idempotent_and_reconciled` | Covered |
| `ORCH-005` | `reconcile` MUST only start missing enabled workers and MUST NOT duplicate running workers. | `src/daemon.rs` (`reconcile_once`, liveness gate before spawn) | `periodic_reconcile_starts_enabled_service` | Covered |
| `ORCH-006` | Liveness/status MUST be determined from recorder control protocol, not only local pid assumptions. | `src/control.rs` (`status`/`stop` protocol calls), `src/daemon.rs` (`build_health`) | `lifecycle_commands_are_idempotent_and_reconciled`, `service_enters_degraded_after_retry_window` | Covered |
| `ORCH-007` | CLI outputs MUST be deterministic and serializable (`RON`/`JSON`/`YAML`). | `src/format.rs`, `src/command.rs` (typed payload printing) | CLI integration tests parse deterministic JSON payloads | Covered |
| `ORCH-008` | Failure modes MUST return stable exit codes and explicit error payloads. | `src/error.rs` (`exit_code`, structured payload), `src/command.rs` error mapping | `missing_action_returns_deterministic_invalid_input_error`, `command_errors_have_stable_codes_exit_codes_and_payloads` | Covered |
| `ORCH-009` | Worker process command construction MUST be deterministic from persisted config. | `src/worker.rs` (`recorder_args`) | `worker::tests::recorder_args_are_deterministic`, `recorder_args_include_required_fields` | Covered |
| `ORCH-010` | V1 MUST support pub/sub recorder workers; other pattern-specific orchestration MAY be added later. | `src/worker.rs` (`publish-subscribe` launch contract) | `recorder_args_include_required_fields` | Covered |

## Phase 6 Decision Matrix

| Decision | Statement | Implementation | Verification | Status |
| --- | --- | --- | --- | --- |
| `DF-001` | Control API transport MUST be request/response service with versioned schema. | `src/control_api.rs`, `src/client.rs`, `src/daemon.rs` | CLI integration tests (client->daemon control flow), `CONTROL_API_VERSION` gating | Covered |
| `DF-002` | Worker model MUST be process-per-recorder for production; in-proc workers MAY be tests-only. | `src/worker.rs` (`ProcessRunner`), `src/daemon.rs` generic over `Runner` | integration tests use process runner behavior | Covered |
| `DF-003` | Identity key MUST be `service + instance + generation` with monotonic per-service generation. | `src/model.rs` (`ServiceSpec` fields), `src/daemon.rs` (`enable` generation bump) | `lifecycle_commands_are_idempotent_and_reconciled` (generation surfaced), state roundtrip tests | Covered |
| `DF-004` | Desired-state store MUST be single durable TOML file with atomic replace and fsync. | `src/state.rs` durable write path | `state::tests::save_and_load_roundtrip` + code path inspection | Covered |
| `DF-005` | Lifecycle semantics MUST support intent (`enable`/`disable`), gating (`pause`/`resume`), and overrides (`start`/`stop`). | `src/cli.rs`, `src/control_api.rs`, `src/daemon.rs` | `lifecycle_commands_are_idempotent_and_reconciled` | Covered |
| `DF-006` | Reconcile MUST run periodically and immediately after accepted commands. | `src/daemon.rs` serve loop interval reconcile + command-path reconcile | `periodic_reconcile_starts_enabled_service`, lifecycle integration tests | Covered |
| `DF-007` | Restart policy MUST use bounded exponential backoff with jitter and degraded-state transition. | `src/model.rs` (`BackoffConfig`), `src/daemon.rs` retry/degraded tracking | `service_enters_degraded_after_retry_window`, `model::tests::backoff_delay_is_bounded` | Covered |
| `DF-008` | Status MUST expose heartbeat freshness, last error, last transition reason, restart counters. | `src/model.rs` (`ServiceHealth`), `src/daemon.rs` (`build_health`) | `service_enters_degraded_after_retry_window` | Covered |
| `DF-009` | Configuration precedence MUST be `CLI > env > config file > profile defaults`. | `src/config.rs` (`resolve`) | `config::tests::cli_overrides_defaults`, `file_env_and_cli_precedence_is_deterministic`, `invalid_numeric_environment_values_are_reported` | Covered |
| `DF-010` | Compatibility policy MUST use canonical commands only and MUST NOT provide legacy aliases. | `src/cli.rs` canonical command set only | CLI help/parse review + integration tests | Covered |

## Notes
- Deferred `DD-*` decisions remain intentionally out of V1 scope.
- High-load soak testing and supervisor orchestration tests remain post-Phase-6 hardening work.
- CI and local coverage are provided by `.github/workflows/ci.yml` and `scripts/coverage.sh`.
