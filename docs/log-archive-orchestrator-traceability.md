# Log Archive Orchestrator Traceability

This matrix maps `ORCH-*` requirements to implementation and verification evidence.

## Requirement Matrix

| Requirement | Statement | Implementation | Verification | Status |
| --- | --- | --- | --- | --- |
| `ORCH-001` | Orchestrator MUST remain userland-only and treat recorder/archive internals as external contracts. | `src/control.rs`, `src/worker.rs` (external binary contract via `iox2-log-control`/`iox2-log-recorder`) | Architecture review + dependency review (`Cargo.toml` has no direct `iceoryx2` runtime dependency) | Covered |
| `ORCH-002` | Desired state MUST be persisted on disk and survive orchestrator restarts. | `src/state.rs` (`load_or_default`, atomic `save`) | `state::tests::save_and_load_roundtrip`, `desired_state_roundtrip_preserves_service_spec` | Covered |
| `ORCH-003` | `enable(service)` MUST be idempotent. | `src/command.rs` (`enable`, `changed` comparison on `ServiceSpec`) | `enable_and_disable_are_idempotent` | Covered |
| `ORCH-004` | `disable(service)` MUST be idempotent. | `src/command.rs` (`disable`, `was_enabled` transition) | `enable_and_disable_are_idempotent` | Covered |
| `ORCH-005` | `reconcile` MUST only start missing enabled workers and MUST NOT duplicate running workers. | `src/command.rs` (`reconcile`, liveness gate before spawn) | `reconcile_starts_missing_enabled_and_avoids_duplicates` | Covered |
| `ORCH-006` | Liveness/status MUST be determined from recorder control protocol, not only local pid assumptions. | `src/control.rs` (`status`/`stop` through `iox2-log-control`) | `reconcile_starts_missing_enabled_and_avoids_duplicates` (behavior driven by control responses) | Covered |
| `ORCH-007` | CLI outputs MUST be deterministic and serializable (`RON`/`JSON`/`YAML`). | `src/format.rs`, `src/command.rs` (`print` + typed payloads) | Unit/integration command-path tests parse JSON payloads | Covered |
| `ORCH-008` | Failure modes MUST return stable exit codes and explicit error payloads. | `src/command.rs` (`CommandError`, `exit_code`, `to_formatted_error`) | `missing_action_returns_deterministic_invalid_input_error` | Covered |
| `ORCH-009` | Worker process command construction MUST be deterministic from persisted config. | `src/worker.rs` (`recorder_args`) | `worker::tests::recorder_args_are_deterministic`, `recorder_args_include_required_fields` | Covered |
| `ORCH-010` | V1 MUST support pub/sub recorder workers; other pattern-specific orchestration MAY be added later. | `src/worker.rs` (`publish-subscribe` launch contract), docs | `recorder_args_include_required_fields` | Covered |

## Notes
- Verification is currently focused on deterministic CLI/control behavior and state transitions.
- High-load and supervisor integration tests are deferred to post-V1 hardening.
