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

use std::process::{Command, Stdio};

use anyhow::Context;

use crate::model::ServiceSpec;

pub trait Runner: Send + Sync + 'static {
    fn spawn_recorder(
        &self,
        recorder_bin: &str,
        service: &str,
        spec: &ServiceSpec,
    ) -> anyhow::Result<u32>;
}

#[derive(Debug, Default, Clone)]
pub struct ProcessRunner;

pub fn recorder_args(service: &str, spec: &ServiceSpec) -> Vec<String> {
    let mut args = vec![
        "--format".to_string(),
        "JSON".to_string(),
        "publish-subscribe".to_string(),
        "--service".to_string(),
        service.to_string(),
        "--storage-path".to_string(),
        spec.storage_path.clone(),
        "--metadata-log-path".to_string(),
        spec.metadata_log_path.clone(),
        "--profile".to_string(),
        spec.profile.as_str().to_string(),
        "--mode".to_string(),
        spec.mode.as_str().to_string(),
        "--cycle-time-ms".to_string(),
        spec.cycle_time_ms.to_string(),
        "--flush-interval-ms".to_string(),
        spec.flush_interval_ms.to_string(),
    ];

    push_optional_u64(&mut args, "--max-disk-bytes", spec.max_disk_bytes);
    if let Some(value) = spec.async_io_backend {
        push_value(&mut args, "--async-io-backend", value.as_str());
    }
    push_optional_u32(
        &mut args,
        "--io-uring-queue-depth",
        spec.io_uring_queue_depth,
    );
    push_optional_u32(&mut args, "--io-submit-batch-max", spec.io_submit_batch_max);
    push_optional_u32(&mut args, "--io-cqe-batch-max", spec.io_cqe_batch_max);
    if let Some(value) = spec.io_uring_register_files {
        push_value(&mut args, "--io-uring-register-files", &value.to_string());
    }
    if let Some(value) = spec.checksum_mode {
        push_value(&mut args, "--checksum-mode", value.as_str());
    }
    if let Some(value) = spec.out_of_space_policy {
        push_value(&mut args, "--out-of-space-policy", value.as_str());
    }
    push_optional_u64(
        &mut args,
        "--metadata-log-roll-bytes",
        spec.metadata_log_roll_bytes,
    );
    push_optional_u64(
        &mut args,
        "--metadata-log-max-bytes",
        spec.metadata_log_max_bytes,
    );

    args
}

fn push_value(args: &mut Vec<String>, flag: &str, value: &str) {
    args.push(flag.to_string());
    args.push(value.to_string());
}

fn push_optional_u32(args: &mut Vec<String>, flag: &str, value: Option<u32>) {
    if let Some(value) = value {
        push_value(args, flag, &value.to_string());
    }
}

fn push_optional_u64(args: &mut Vec<String>, flag: &str, value: Option<u64>) {
    if let Some(value) = value {
        push_value(args, flag, &value.to_string());
    }
}

impl Runner for ProcessRunner {
    fn spawn_recorder(
        &self,
        recorder_bin: &str,
        service: &str,
        spec: &ServiceSpec,
    ) -> anyhow::Result<u32> {
        let args = recorder_args(service, spec);
        let child = Command::new(recorder_bin)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| {
                format!(
                    "failed to spawn '{}' for service '{}'",
                    recorder_bin, service
                )
            })?;

        Ok(child.id())
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{PersistenceMode, RecorderProfile, ServiceSpec};

    use super::*;

    #[test]
    fn recorder_args_are_deterministic() {
        let spec = ServiceSpec {
            enabled: true,
            paused: false,
            instance: "default".to_string(),
            generation: 1,
            storage_path: "/tmp/storage".to_string(),
            metadata_log_path: "/tmp/metadata".to_string(),
            profile: RecorderProfile::Throughput,
            mode: PersistenceMode::Async,
            cycle_time_ms: 5,
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
        };

        let args = recorder_args("My/Service", &spec);
        assert_eq!(args[0], "--format");
        assert_eq!(args[2], "publish-subscribe");
        assert!(args.contains(&"throughput".to_string()));
        assert!(args.contains(&"async".to_string()));
    }
}
