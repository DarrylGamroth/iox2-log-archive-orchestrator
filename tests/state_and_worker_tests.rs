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

use iceoryx2_userland_log_archive_orchestrator::model::{
    AsyncIoBackend, ChecksumMode, DesiredState, OutOfSpacePolicy, PersistenceMode, RecorderProfile,
    ServiceSpec,
};
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
            paused: false,
            instance: "default".to_string(),
            generation: 1,
            storage_path: "/tmp/storage".to_string(),
            metadata_log_path: "/tmp/metadata".to_string(),
            profile: RecorderProfile::Throughput,
            mode: PersistenceMode::Async,
            cycle_time_ms: 5,
            flush_interval_ms: 50,
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

    save(&path, &state).unwrap();
    let loaded = load_or_default(&path).unwrap();
    assert_eq!(loaded.services.len(), 1);
    assert_eq!(loaded.services["My/Camera/Service"].cycle_time_ms, 5);
}

#[test]
fn recorder_args_include_required_fields() {
    let spec = ServiceSpec {
        enabled: true,
        paused: false,
        instance: "default".to_string(),
        generation: 1,
        storage_path: "/tmp/storage".to_string(),
        metadata_log_path: "/tmp/metadata".to_string(),
        profile: RecorderProfile::Balanced,
        mode: PersistenceMode::Async,
        cycle_time_ms: 10,
        flush_interval_ms: 100,
        max_disk_bytes: Some(1024 * 1024),
        async_io_backend: Some(AsyncIoBackend::Blocking),
        io_uring_queue_depth: Some(64),
        io_submit_batch_max: Some(16),
        io_cqe_batch_max: Some(32),
        io_uring_register_files: Some(false),
        checksum_mode: Some(ChecksumMode::Crc32c),
        out_of_space_policy: Some(OutOfSpacePolicy::FailWriter),
        metadata_log_roll_bytes: Some(1024 * 1024),
        metadata_log_max_bytes: Some(16 * 1024 * 1024),
    };

    let args = recorder_args("A/B/C", &spec);
    assert!(args.contains(&"publish-subscribe".to_string()));
    assert!(args.contains(&"--service".to_string()));
    assert!(args.contains(&"A/B/C".to_string()));
    assert!(args.contains(&"--storage-path".to_string()));
    assert!(args.contains(&"--metadata-log-path".to_string()));
    assert!(args.contains(&"--async-io-backend".to_string()));
    assert!(args.contains(&"blocking".to_string()));
    assert!(args.contains(&"--io-uring-register-files".to_string()));
    assert!(args.contains(&"false".to_string()));
    assert!(args.contains(&"--metadata-log-roll-bytes".to_string()));
}
