// Copyright (c) 2026 Contributors to the Eclipse Foundation
//
// See the NOTICE file(s) distributed with this work for additional
// information regarding copyright ownership.
//
// This program and the accompanying materials are made available under the
// terms of the Apache Software License 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0, or the MIT license
// which is available at https://opensource.org/licenses/MIT.
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::process::Command;

use anyhow::Context;
use serde::Deserialize;

use crate::model::ServiceLiveStatus;

#[derive(Debug, Clone)]
pub struct ControlClient {
    pub control_bin: String,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct ControlStatusPayload {
    is_paused: bool,
    dropped_while_paused: u64,
    paused_since_ns: Option<u64>,
    committed_records: u64,
    payload_bytes_committed: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct ErrorPayload {
    message: String,
}

impl ControlClient {
    pub fn status(&self, service: &str) -> anyhow::Result<ServiceLiveStatus> {
        let output = Command::new(&self.control_bin)
            .arg("--format")
            .arg("JSON")
            .arg("status")
            .arg("--service")
            .arg(service)
            .arg("--timeout-ms")
            .arg(self.timeout_ms.to_string())
            .output()
            .with_context(|| format!("failed to execute '{}'", self.control_bin))?;

        if output.status.success() {
            let status: ControlStatusPayload = serde_json::from_slice(&output.stdout)
                .context("failed to parse iox2-log-control status JSON")?;
            Ok(ServiceLiveStatus {
                available: true,
                is_paused: Some(status.is_paused),
                dropped_while_paused: Some(status.dropped_while_paused),
                paused_since_ns: status.paused_since_ns,
                committed_records: Some(status.committed_records),
                payload_bytes_committed: Some(status.payload_bytes_committed),
                message: None,
            })
        } else {
            let message = match serde_json::from_slice::<ErrorPayload>(&output.stderr) {
                Ok(error) => error.message,
                Err(_) => String::from_utf8_lossy(&output.stderr).to_string(),
            };
            Ok(ServiceLiveStatus {
                available: false,
                is_paused: None,
                dropped_while_paused: None,
                paused_since_ns: None,
                committed_records: None,
                payload_bytes_committed: None,
                message: Some(message.trim().to_string()),
            })
        }
    }

    pub fn stop(&self, service: &str) -> anyhow::Result<ServiceLiveStatus> {
        let output = Command::new(&self.control_bin)
            .arg("--format")
            .arg("JSON")
            .arg("stop")
            .arg("--service")
            .arg(service)
            .arg("--timeout-ms")
            .arg(self.timeout_ms.to_string())
            .output()
            .with_context(|| format!("failed to execute '{}'", self.control_bin))?;

        if output.status.success() {
            self.status(service)
        } else {
            let message = match serde_json::from_slice::<ErrorPayload>(&output.stderr) {
                Ok(error) => error.message,
                Err(_) => String::from_utf8_lossy(&output.stderr).to_string(),
            };
            Ok(ServiceLiveStatus {
                available: false,
                is_paused: None,
                dropped_while_paused: None,
                paused_since_ns: None,
                committed_records: None,
                payload_bytes_committed: None,
                message: Some(message.trim().to_string()),
            })
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs;
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::time::Duration;

    use super::ControlClient;

    fn write_script(path: &Path, body: &str) {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .unwrap();
        file.write_all(body.as_bytes()).unwrap();
        file.sync_all().unwrap();
        drop(file);
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
        std::thread::sleep(Duration::from_millis(20));
    }

    #[test]
    fn status_success_parses_control_payload() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("control.sh");
        write_script(
            &script,
            r#"#!/usr/bin/env bash
set -euo pipefail
echo '{"is_paused":true,"dropped_while_paused":3,"paused_since_ns":123,"committed_records":42,"payload_bytes_committed":4096}'
"#,
        );

        let client = ControlClient {
            control_bin: script.to_string_lossy().to_string(),
            timeout_ms: 7,
        };
        let status = client.status("Camera/A").unwrap();
        assert!(status.available);
        assert_eq!(status.is_paused, Some(true));
        assert_eq!(status.dropped_while_paused, Some(3));
        assert_eq!(status.paused_since_ns, Some(123));
        assert_eq!(status.committed_records, Some(42));
        assert_eq!(status.payload_bytes_committed, Some(4096));
    }

    #[test]
    fn status_failure_prefers_structured_error_payload() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("control.sh");
        write_script(
            &script,
            r#"#!/usr/bin/env bash
set -euo pipefail
echo '{"message":"service unavailable"}' >&2
exit 3
"#,
        );

        let client = ControlClient {
            control_bin: script.to_string_lossy().to_string(),
            timeout_ms: 7,
        };
        let status = client.status("Camera/A").unwrap();
        assert!(!status.available);
        assert_eq!(status.message.as_deref(), Some("service unavailable"));
    }

    #[test]
    fn stop_failure_falls_back_to_plain_stderr() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("control.sh");
        write_script(
            &script,
            r#"#!/usr/bin/env bash
set -euo pipefail
echo 'plain stop failure' >&2
exit 3
"#,
        );

        let client = ControlClient {
            control_bin: script.to_string_lossy().to_string(),
            timeout_ms: 7,
        };
        let status = client.stop("Camera/A").unwrap();
        assert!(!status.available);
        assert_eq!(status.message.as_deref(), Some("plain stop failure"));
    }
}
