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
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output};
use std::thread;
use std::time::{Duration, Instant};

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
if [[ -f "${mock_dir}/fail_service" ]] && [[ "$(cat "${mock_dir}/fail_service")" == "$service" ]]; then
  exit 1
fi
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

struct DaemonGuard {
    child: Child,
    state_path: PathBuf,
    control_service: String,
    recorder_bin: String,
    control_bin: String,
    mock_dir: PathBuf,
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        let _ = run_orchestrator(
            &self.state_path,
            &self.control_service,
            &self.recorder_bin,
            &self.control_bin,
            &self.mock_dir,
            &["shutdown"],
        );
        let timeout = Instant::now() + Duration::from_secs(2);
        while Instant::now() < timeout {
            if self.child.try_wait().ok().flatten().is_some() {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn wait_for_daemon(
    state_path: &Path,
    control_service: &str,
    recorder_bin: &str,
    control_bin: &str,
    mock_dir: &Path,
    timeout: Duration,
) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let output = run_orchestrator(
            state_path,
            control_service,
            recorder_bin,
            control_bin,
            mock_dir,
            &["daemon-status"],
        );
        if output.status.success() {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("timed out waiting for orchestrator daemon control service");
}

fn run_orchestrator(
    state_path: &Path,
    control_service: &str,
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
    .arg("--control-service")
    .arg(control_service)
    .arg("--recorder-bin")
    .arg(recorder_bin)
    .arg("--control-bin")
    .arg(control_bin)
    .args(args)
    .env("MOCK_DIR", mock_dir)
    .output()
    .unwrap()
}

fn start_daemon(
    state_path: &Path,
    control_service: &str,
    recorder_bin: &str,
    control_bin: &str,
    mock_dir: &Path,
    extra_args: &[&str],
) -> DaemonGuard {
    let mut command = Command::new(env!(
        "CARGO_BIN_EXE_iceoryx2-userland-log-archive-orchestrator"
    ));
    command
        .arg("--format")
        .arg("json")
        .arg("--state-path")
        .arg(state_path)
        .arg("--control-service")
        .arg(control_service)
        .arg("--recorder-bin")
        .arg(recorder_bin)
        .arg("--control-bin")
        .arg(control_bin)
        .args(extra_args)
        .arg("serve")
        .env("MOCK_DIR", mock_dir);

    let child = command.spawn().unwrap();
    wait_for_daemon(
        state_path,
        control_service,
        recorder_bin,
        control_bin,
        mock_dir,
        Duration::from_secs(3),
    );
    DaemonGuard {
        child,
        state_path: state_path.to_path_buf(),
        control_service: control_service.to_string(),
        recorder_bin: recorder_bin.to_string(),
        control_bin: control_bin.to_string(),
        mock_dir: mock_dir.to_path_buf(),
    }
}

fn parse_json_stdout(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap()
}

fn unique_control_service(test_name: &str) -> String {
    format!(
        "iox2/log/archive/orchestrator/test/{}/{}",
        std::process::id(),
        test_name
    )
}

fn wait_for_file(path: &Path, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if path.exists() {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("timed out waiting for file {}", path.display());
}

#[test]
fn lifecycle_commands_are_idempotent_and_reconciled() {
    let dir = tempfile::tempdir().unwrap();
    let state_path = dir.path().join("state.toml");
    let control_service = unique_control_service("lifecycle");
    let (recorder_bin, control_bin) = create_mock_bins(dir.path());
    let _daemon = start_daemon(
        &state_path,
        &control_service,
        &recorder_bin,
        &control_bin,
        dir.path(),
        &["--reconcile-interval-ms", "50"],
    );

    let first_enable = run_orchestrator(
        &state_path,
        &control_service,
        &recorder_bin,
        &control_bin,
        dir.path(),
        &[
            "enable",
            "--service",
            "svc_a",
            "--instance",
            "inst0",
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
    assert_eq!(first_enable_json["generation"], 1);
    assert_eq!(first_enable_json["started"], true);

    let second_enable = run_orchestrator(
        &state_path,
        &control_service,
        &recorder_bin,
        &control_bin,
        dir.path(),
        &[
            "enable",
            "--service",
            "svc_a",
            "--instance",
            "inst0",
            "--storage-path",
            "/tmp/storage_a",
            "--metadata-log-path",
            "/tmp/meta_a",
        ],
    );
    assert!(second_enable.status.success(), "{second_enable:?}");
    let second_enable_json = parse_json_stdout(&second_enable);
    assert_eq!(second_enable_json["changed"], false);
    assert_eq!(second_enable_json["started"], false);

    let pause = run_orchestrator(
        &state_path,
        &control_service,
        &recorder_bin,
        &control_bin,
        dir.path(),
        &["pause", "--service", "svc_a"],
    );
    assert!(pause.status.success(), "{pause:?}");
    let pause_json = parse_json_stdout(&pause);
    assert_eq!(pause_json["changed"], true);
    assert_eq!(pause_json["stop_requested"], true);

    let resume = run_orchestrator(
        &state_path,
        &control_service,
        &recorder_bin,
        &control_bin,
        dir.path(),
        &["resume", "--service", "svc_a"],
    );
    assert!(resume.status.success(), "{resume:?}");
    let resume_json = parse_json_stdout(&resume);
    assert_eq!(resume_json["changed"], true);
    assert_eq!(resume_json["started"], true);

    let first_disable = run_orchestrator(
        &state_path,
        &control_service,
        &recorder_bin,
        &control_bin,
        dir.path(),
        &["disable", "--service", "svc_a"],
    );
    assert!(first_disable.status.success(), "{first_disable:?}");
    let first_disable_json = parse_json_stdout(&first_disable);
    assert_eq!(first_disable_json["changed"], true);
    assert_eq!(first_disable_json["stop_requested"], true);

    let second_disable = run_orchestrator(
        &state_path,
        &control_service,
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
fn periodic_reconcile_starts_enabled_service() {
    let dir = tempfile::tempdir().unwrap();
    let state_path = dir.path().join("state.toml");
    let control_service = unique_control_service("periodic");
    let (recorder_bin, control_bin) = create_mock_bins(dir.path());

    let mut state = DesiredState::new();
    state.services.insert(
        "svc_start".to_string(),
        ServiceSpec {
            enabled: true,
            paused: false,
            instance: "inst0".to_string(),
            generation: 1,
            storage_path: "/tmp/storage_start".to_string(),
            metadata_log_path: "/tmp/meta_start".to_string(),
            profile: RecorderProfile::Balanced,
            mode: PersistenceMode::Async,
            cycle_time_ms: 10,
            flush_interval_ms: 100,
            max_disk_bytes: None,
            async_io_backend: None,
            io_uring_queue_depth: None,
            io_submit_batch_max: None,
            io_cqe_batch_max: None,
            io_uring_register_files: None,
            checksum_mode: None,
            out_of_space_policy: None,
            metadata_log_roll_bytes: None,
            metadata_log_max_bytes: None,
        },
    );
    save(&state_path, &state).unwrap();

    let _daemon = start_daemon(
        &state_path,
        &control_service,
        &recorder_bin,
        &control_bin,
        dir.path(),
        &["--reconcile-interval-ms", "20"],
    );

    wait_for_file(
        &dir.path().join("spawn_count_svc_start"),
        Duration::from_secs(2),
    );
    let spawn_count = fs::read_to_string(dir.path().join("spawn_count_svc_start")).unwrap();
    assert_eq!(spawn_count.trim(), "1");
}

#[test]
fn service_enters_degraded_after_retry_window() {
    let dir = tempfile::tempdir().unwrap();
    let state_path = dir.path().join("state.toml");
    let control_service = unique_control_service("degraded");
    let (recorder_bin, control_bin) = create_mock_bins(dir.path());
    fs::write(dir.path().join("fail_service"), "svc_fail").unwrap();

    let _daemon = start_daemon(
        &state_path,
        &control_service,
        &recorder_bin,
        &control_bin,
        dir.path(),
        &[
            "--reconcile-interval-ms",
            "20",
            "--backoff-initial-ms",
            "10",
            "--backoff-max-ms",
            "10",
            "--backoff-jitter-percent",
            "0",
            "--backoff-max-window-ms",
            "100",
        ],
    );

    let enable = run_orchestrator(
        &state_path,
        &control_service,
        &recorder_bin,
        &control_bin,
        dir.path(),
        &[
            "enable",
            "--service",
            "svc_fail",
            "--instance",
            "inst0",
            "--storage-path",
            "/tmp/storage_fail",
            "--metadata-log-path",
            "/tmp/meta_fail",
        ],
    );
    assert!(enable.status.success(), "{enable:?}");

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let status = run_orchestrator(
            &state_path,
            &control_service,
            &recorder_bin,
            &control_bin,
            dir.path(),
            &["status", "--service", "svc_fail"],
        );
        assert!(status.status.success(), "{status:?}");
        let status_json = parse_json_stdout(&status);
        assert_eq!(status_json["operation"], "status");
        assert_eq!(status_json["configured"], true);
        assert_eq!(status_json["enabled"], true);

        let degraded = status_json["health"]["degraded"].as_bool().unwrap_or(false);
        let attempts = status_json["health"]["restart_attempts"]
            .as_u64()
            .unwrap_or_default();
        if degraded {
            assert!(attempts > 0);
            break;
        }
        if Instant::now() >= deadline {
            panic!("service did not enter degraded state within timeout: {status_json}");
        }
        thread::sleep(Duration::from_millis(30));
    }
}

#[test]
fn missing_action_returns_deterministic_invalid_input_error() {
    let output = Command::new(env!(
        "CARGO_BIN_EXE_iceoryx2-userland-log-archive-orchestrator"
    ))
    .arg("--format")
    .arg("json")
    .output()
    .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error_code"], "InvalidInput");
}
