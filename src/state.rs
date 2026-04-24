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

use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use anyhow::Context;

use crate::model::DesiredState;

pub fn load_or_default(path: &Path) -> anyhow::Result<DesiredState> {
    if !path.exists() {
        return Ok(DesiredState::new());
    }

    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read desired-state file {}", path.display()))?;
    let state: DesiredState = toml::from_str(&content)
        .with_context(|| format!("failed to parse desired-state file {}", path.display()))?;
    state.validate()?;
    Ok(state)
}

pub fn save(path: &Path, state: &DesiredState) -> anyhow::Result<()> {
    state.validate()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    let encoded = toml::to_string_pretty(state).context("failed to encode desired-state TOML")?;
    let tmp_path = path.with_extension("tmp");

    let mut tmp_file = File::create(&tmp_path).with_context(|| {
        format!(
            "failed to create temp desired-state file {}",
            tmp_path.display()
        )
    })?;
    tmp_file.write_all(encoded.as_bytes()).with_context(|| {
        format!(
            "failed to write temp desired-state file {}",
            tmp_path.display()
        )
    })?;
    tmp_file.sync_all().with_context(|| {
        format!(
            "failed to fsync temp desired-state file {}",
            tmp_path.display()
        )
    })?;
    fs::rename(&tmp_path, path).with_context(|| {
        format!(
            "failed to atomically replace desired-state file {}",
            path.display()
        )
    })?;
    let final_file = File::open(path).with_context(|| {
        format!(
            "failed to open desired-state file for fsync {}",
            path.display()
        )
    })?;
    final_file
        .sync_all()
        .with_context(|| format!("failed to fsync desired-state file {}", path.display()))?;
    if let Some(parent) = path.parent() {
        let parent_dir = File::open(parent).with_context(|| {
            format!("failed to open state parent directory {}", parent.display())
        })?;
        parent_dir.sync_all().with_context(|| {
            format!(
                "failed to fsync state parent directory {}",
                parent.display()
            )
        })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::model::{DesiredState, ServiceSpec};

    use super::*;

    #[test]
    fn load_or_default_returns_new_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.toml");

        let state = load_or_default(&path).unwrap();
        assert_eq!(state.version, crate::model::DESIRED_STATE_VERSION);
        assert!(state.services.is_empty());
    }

    #[test]
    fn save_and_load_roundtrip() {
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
                profile: Default::default(),
                mode: Default::default(),
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

        save(&path, &state).unwrap();
        let loaded = load_or_default(&path).unwrap();
        assert_eq!(loaded.services.len(), 1);
        assert!(loaded.services["My/Camera/Service"].enabled);
    }
}
