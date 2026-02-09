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

use iceoryx2_userland_log_archive_orchestrator::model::{DesiredState, PersistenceMode, RecorderProfile, ServiceSpec};
use iceoryx2_userland_log_archive_orchestrator::state::{load_or_default, save};
use iceoryx2_userland_log_archive_orchestrator::worker::recorder_args;

#[test]
fn desired_state_roundtrip_preserves_service_spec() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.toml");

    let mut state = DesiredState::new();
    state.services.insert(
        "My/Camera/Service".to_string(),
        ServiceSpec {
            enabled: true,
            storage_path: "/tmp/storage".to_string(),
            metadata_log_path: "/tmp/metadata".to_string(),
            profile: RecorderProfile::Throughput,
            mode: PersistenceMode::Async,
            cycle_time_ms: 5,
            flush_interval_ms: 50,
        },
    );

    save(&path, &state).unwrap();
    let loaded = load_or_default(&path).unwrap();
    assert_eq!(loaded.services.len(), 1);
    assert_eq!(loaded.services["My/Camera/Service"].cycle_time_ms, 5);
}

#[test]
fn recorder_args_include_required_fields() {
    let spec = ServiceSpec {
        enabled: true,
        storage_path: "/tmp/storage".to_string(),
        metadata_log_path: "/tmp/metadata".to_string(),
        profile: RecorderProfile::Balanced,
        mode: PersistenceMode::Async,
        cycle_time_ms: 10,
        flush_interval_ms: 100,
    };

    let args = recorder_args("A/B/C", &spec);
    assert!(args.contains(&"publish-subscribe".to_string()));
    assert!(args.contains(&"--service".to_string()));
    assert!(args.contains(&"A/B/C".to_string()));
    assert!(args.contains(&"--storage-path".to_string()));
    assert!(args.contains(&"--metadata-log-path".to_string()));
}
