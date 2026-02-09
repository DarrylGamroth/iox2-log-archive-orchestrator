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

use std::collections::BTreeMap;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

pub const DESIRED_STATE_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ValueEnum, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RecorderProfile {
    Durable,
    #[default]
    Balanced,
    Throughput,
    Replay,
}

impl RecorderProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Durable => "durable",
            Self::Balanced => "balanced",
            Self::Throughput => "throughput",
            Self::Replay => "replay",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ValueEnum, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PersistenceMode {
    Volatile,
    #[default]
    Async,
    Sync,
}

impl PersistenceMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Volatile => "volatile",
            Self::Async => "async",
            Self::Sync => "sync",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceSpec {
    pub enabled: bool,
    pub storage_path: String,
    pub metadata_log_path: String,
    #[serde(default)]
    pub profile: RecorderProfile,
    #[serde(default)]
    pub mode: PersistenceMode,
    #[serde(default = "default_cycle_time_ms")]
    pub cycle_time_ms: u64,
    #[serde(default = "default_flush_interval_ms")]
    pub flush_interval_ms: u64,
}

const fn default_cycle_time_ms() -> u64 {
    10
}

const fn default_flush_interval_ms() -> u64 {
    100
}

impl ServiceSpec {
    pub fn validate(&self, service: &str) -> anyhow::Result<()> {
        if service.trim().is_empty() {
            anyhow::bail!("service name must not be empty")
        }
        if self.storage_path.trim().is_empty() {
            anyhow::bail!("storage_path for service '{}' must not be empty", service)
        }
        if self.metadata_log_path.trim().is_empty() {
            anyhow::bail!("metadata_log_path for service '{}' must not be empty", service)
        }
        if self.cycle_time_ms == 0 {
            anyhow::bail!("cycle_time_ms for service '{}' must be > 0", service)
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DesiredState {
    pub version: u32,
    #[serde(default)]
    pub services: BTreeMap<String, ServiceSpec>,
}

impl DesiredState {
    pub fn new() -> Self {
        Self {
            version: DESIRED_STATE_VERSION,
            services: BTreeMap::new(),
        }
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        if self.version != DESIRED_STATE_VERSION {
            anyhow::bail!(
                "unsupported desired-state version {}, expected {}",
                self.version,
                DESIRED_STATE_VERSION
            )
        }

        for (service, spec) in &self.services {
            spec.validate(service)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceLiveStatus {
    pub available: bool,
    pub is_paused: Option<bool>,
    pub dropped_while_paused: Option<u64>,
    pub paused_since_ns: Option<u64>,
    pub committed_records: Option<u64>,
    pub payload_bytes_committed: Option<u64>,
    pub message: Option<String>,
}
