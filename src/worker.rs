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

pub fn recorder_args(service: &str, spec: &ServiceSpec) -> Vec<String> {
    vec![
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
    ]
}

pub fn spawn_recorder(recorder_bin: &str, service: &str, spec: &ServiceSpec) -> anyhow::Result<u32> {
    let args = recorder_args(service, spec);
    let child = Command::new(recorder_bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("failed to spawn '{}' for service '{}'", recorder_bin, service))?;

    Ok(child.id())
}

#[cfg(test)]
mod tests {
    use crate::model::{PersistenceMode, RecorderProfile, ServiceSpec};

    use super::*;

    #[test]
    fn recorder_args_are_deterministic() {
        let spec = ServiceSpec {
            enabled: true,
            storage_path: "/tmp/storage".to_string(),
            metadata_log_path: "/tmp/metadata".to_string(),
            profile: RecorderProfile::Throughput,
            mode: PersistenceMode::Async,
            cycle_time_ms: 5,
            flush_interval_ms: 100,
        };

        let args = recorder_args("My/Service", &spec);
        assert_eq!(args[0], "--format");
        assert_eq!(args[2], "publish-subscribe");
        assert!(args.contains(&"throughput".to_string()));
        assert!(args.contains(&"async".to_string()));
    }
}
