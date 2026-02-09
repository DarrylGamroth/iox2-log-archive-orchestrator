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
