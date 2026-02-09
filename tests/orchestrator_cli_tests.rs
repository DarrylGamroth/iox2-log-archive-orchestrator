// Copyright (c) 2026 Contributors to the Eclipse Foundation
//
// See the NOTICE file(s) distributed with this work for additional
// information regarding copyright ownership.
//
// This program and the accompanying materials are made available under the
// terms of the Apache Software License 2.0 which is available at
// https://opensource.org/licenses/MIT or the Apache Software License 2.0
// which is available at https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};

use iceoryx2_userland_log_archive_orchestrator::model::{
    DesiredState, PersistenceMode, RecorderProfile, ServiceSpec,
};
use iceoryx2_userland_log_archive_orchestrator::state::save;

fn write_script(path: &Path, content: &str) {
    fs::write(path, content).unwrap();
    let mut perms = fs::metadata(path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).unwrap();
}

fn create_mock_bins(mock_dir: &Path) -> (String, String) {
    let recorder = mock_dir.join("mock-recorder.sh");
    let control = mock_dir.join("mock-control.sh");

    write_script(
        &recorder,
        r#"#!/usr/bin/env bash
set -euo pipefail
service=""
for ((i=1; i<=$#; i++)); do
  arg="${!i}"
  if [[ "$arg" == "--service" ]]; then
    j=$((i+1))
    service="${!j}"
  fi
done
if [[ -z "$service" ]]; then
  exit 2
fi
mock_dir="${MOCK_DIR:?}"
touch "${mock_dir}/live_${service}"
count_file="${mock_dir}/spawn_count_${service}"
if [[ -f "$count_file" ]]; then
  count=$(cat "$count_file")
else
  count=0
fi
echo $((count + 1)) > "$count_file"
exit 0
"#,
    );

    write_script(
        &control,
        r#"#!/usr/bin/env bash
set -euo pipefail
service=""
action=""
for ((i=1; i<=$#; i++)); do
  arg="${!i}"
  if [[ "$arg" == "status" || "$arg" == "stop" ]]; then
    action="$arg"
  fi
  if [[ "$arg" == "--service" ]]; then
    j=$((i+1))
    service="${!j}"
  fi
done
if [[ -z "$service" || -z "$action" ]]; then
  echo '{"message":"invalid invocation"}' >&2
  exit 2
fi
mock_dir="${MOCK_DIR:?}"
live_file="${mock_dir}/live_${service}"
if [[ "$action" == "status" ]]; then
  if [[ -f "$live_file" ]]; then
    echo '{"is_paused":false,"dropped_while_paused":0,"paused_since_ns":null,"committed_records":10,"payload_bytes_committed":1024}'
    exit 0
  fi
  echo '{"message":"not available"}' >&2
  exit 3
fi
if [[ "$action" == "stop" ]]; then
  rm -f "$live_file"
  echo '{"operation":"stop"}'
  exit 0
fi
"#,
    );

    (
        recorder.to_string_lossy().to_string(),
        control.to_string_lossy().to_string(),
    )
}

fn run_orchestrator(
    state_path: &Path,
    recorder_bin: &str,
    control_bin: &str,
    mock_dir: &Path,
    args: &[&str],
) -> Output {
    Command::new(env!(
        "CARGO_BIN_EXE_iceoryx2-userland-log-archive-orchestrator"
    ))
    .arg("--format")
    .arg("json")
    .arg("--state-path")
    .arg(state_path)
    .arg("--recorder-bin")
    .arg(recorder_bin)
    .arg("--control-bin")
    .arg(control_bin)
    .args(args)
    .env("MOCK_DIR", mock_dir)
    .output()
    .unwrap()
}

fn parse_json_stdout(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn enable_and_disable_are_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let state_path = dir.path().join("state.toml");
    let (recorder_bin, control_bin) = create_mock_bins(dir.path());

    let first_enable = run_orchestrator(
        &state_path,
        &recorder_bin,
        &control_bin,
        dir.path(),
        &[
            "enable",
            "--service",
            "svc_a",
            "--storage-path",
            "/tmp/storage_a",
            "--metadata-log-path",
            "/tmp/meta_a",
        ],
    );
    assert!(first_enable.status.success(), "{first_enable:?}");
    let first_enable_json = parse_json_stdout(&first_enable);
    assert_eq!(first_enable_json["operation"], "enable");
    assert_eq!(first_enable_json["changed"], true);
    assert!(first_enable_json["launched_pid"].as_u64().is_some());

    let second_enable = run_orchestrator(
        &state_path,
        &recorder_bin,
        &control_bin,
        dir.path(),
        &[
            "enable",
            "--service",
            "svc_a",
            "--storage-path",
            "/tmp/storage_a",
            "--metadata-log-path",
            "/tmp/meta_a",
        ],
    );
    assert!(second_enable.status.success(), "{second_enable:?}");
    let second_enable_json = parse_json_stdout(&second_enable);
    assert_eq!(second_enable_json["changed"], false);
    assert!(second_enable_json["launched_pid"].is_null());

    let first_disable = run_orchestrator(
        &state_path,
        &recorder_bin,
        &control_bin,
        dir.path(),
        &["disable", "--service", "svc_a"],
    );
    assert!(first_disable.status.success(), "{first_disable:?}");
    let first_disable_json = parse_json_stdout(&first_disable);
    assert_eq!(first_disable_json["operation"], "disable");
    assert_eq!(first_disable_json["changed"], true);
    assert_eq!(first_disable_json["stop_requested"], true);

    let second_disable = run_orchestrator(
        &state_path,
        &recorder_bin,
        &control_bin,
        dir.path(),
        &["disable", "--service", "svc_a"],
    );
    assert!(second_disable.status.success(), "{second_disable:?}");
    let second_disable_json = parse_json_stdout(&second_disable);
    assert_eq!(second_disable_json["changed"], false);
    assert_eq!(second_disable_json["stop_requested"], false);
}

#[test]
fn reconcile_starts_missing_enabled_and_avoids_duplicates() {
    let dir = tempfile::tempdir().unwrap();
    let state_path = dir.path().join("state.toml");
    let (recorder_bin, control_bin) = create_mock_bins(dir.path());

    let mut state = DesiredState::new();
    state.services.insert(
        "svc_start".to_string(),
        ServiceSpec {
            enabled: true,
            storage_path: "/tmp/storage_start".to_string(),
            metadata_log_path: "/tmp/meta_start".to_string(),
            profile: RecorderProfile::Balanced,
            mode: PersistenceMode::Async,
            cycle_time_ms: 10,
            flush_interval_ms: 100,
        },
    );
    state.services.insert(
        "svc_running".to_string(),
        ServiceSpec {
            enabled: true,
            storage_path: "/tmp/storage_running".to_string(),
            metadata_log_path: "/tmp/meta_running".to_string(),
            profile: RecorderProfile::Balanced,
            mode: PersistenceMode::Async,
            cycle_time_ms: 10,
            flush_interval_ms: 100,
        },
    );
    state.services.insert(
        "svc_stop".to_string(),
        ServiceSpec {
            enabled: false,
            storage_path: "/tmp/storage_stop".to_string(),
            metadata_log_path: "/tmp/meta_stop".to_string(),
            profile: RecorderProfile::Balanced,
            mode: PersistenceMode::Async,
            cycle_time_ms: 10,
            flush_interval_ms: 100,
        },
    );
    save(&state_path, &state).unwrap();

    fs::write(dir.path().join("live_svc_running"), b"1").unwrap();
    fs::write(dir.path().join("live_svc_stop"), b"1").unwrap();

    let reconcile = run_orchestrator(
        &state_path,
        &recorder_bin,
        &control_bin,
        dir.path(),
        &["reconcile"],
    );
    assert!(reconcile.status.success(), "{reconcile:?}");

    let reconcile_json = parse_json_stdout(&reconcile);
    assert_eq!(reconcile_json["operation"], "reconcile");

    let started = reconcile_json["started_services"].as_array().unwrap();
    let already_running = reconcile_json["already_running_services"]
        .as_array()
        .unwrap();
    let stopped = reconcile_json["stopped_services"].as_array().unwrap();

    assert_eq!(started.len(), 1);
    assert_eq!(started[0], "svc_start");
    assert_eq!(already_running.len(), 1);
    assert_eq!(already_running[0], "svc_running");
    assert_eq!(stopped.len(), 1);
    assert_eq!(stopped[0], "svc_stop");

    let spawn_count_start = fs::read_to_string(dir.path().join("spawn_count_svc_start")).unwrap();
    assert_eq!(spawn_count_start.trim(), "1");
    assert!(!dir.path().join("live_svc_stop").exists());
}

#[test]
fn missing_action_returns_deterministic_invalid_input_error() {
    let dir = tempfile::tempdir().unwrap();
    let state_path = dir.path().join("state.toml");
    let (recorder_bin, control_bin) = create_mock_bins(dir.path());

    let output = Command::new(env!(
        "CARGO_BIN_EXE_iceoryx2-userland-log-archive-orchestrator"
    ))
    .arg("--format")
    .arg("json")
    .arg("--state-path")
    .arg(&state_path)
    .arg("--recorder-bin")
    .arg(&recorder_bin)
    .arg("--control-bin")
    .arg(&control_bin)
    .env("MOCK_DIR", dir.path())
    .output()
    .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error_code"], "InvalidInput");
}
