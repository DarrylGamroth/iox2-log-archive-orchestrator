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
use std::time::Duration;
use std::time::Instant;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

pub const DESIRED_STATE_VERSION: u32 = 1;
pub const CONTROL_API_VERSION: u16 = 1;

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
    #[serde(default)]
    pub paused: bool,
    #[serde(default = "default_instance")]
    pub instance: String,
    #[serde(default = "default_generation")]
    pub generation: u64,
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

fn default_instance() -> String {
    "default".to_string()
}

const fn default_generation() -> u64 {
    1
}

impl ServiceSpec {
    pub fn validate(&self, service: &str) -> anyhow::Result<()> {
        if service.trim().is_empty() {
            anyhow::bail!("service name must not be empty")
        }
        if self.instance.trim().is_empty() {
            anyhow::bail!("instance for service '{}' must not be empty", service)
        }
        if self.generation == 0 {
            anyhow::bail!("generation for service '{}' must be > 0", service)
        }
        if self.storage_path.trim().is_empty() {
            anyhow::bail!("storage_path for service '{}' must not be empty", service)
        }
        if self.metadata_log_path.trim().is_empty() {
            anyhow::bail!(
                "metadata_log_path for service '{}' must not be empty",
                service
            )
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceLiveStatus {
    pub available: bool,
    pub is_paused: Option<bool>,
    pub dropped_while_paused: Option<u64>,
    pub paused_since_ns: Option<u64>,
    pub committed_records: Option<u64>,
    pub payload_bytes_committed: Option<u64>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceIdentity {
    pub service: String,
    pub instance: String,
    pub generation: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceHealth {
    pub identity: ServiceIdentity,
    pub live: ServiceLiveStatus,
    pub heartbeat_age_ms: Option<u64>,
    pub last_error: Option<String>,
    pub last_transition_reason: Option<String>,
    pub restart_attempts: u32,
    pub degraded: bool,
    pub next_retry_in_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct RetryTracker {
    pub attempts: u32,
    pub degraded: bool,
    pub first_failure_at: Option<Instant>,
    pub next_retry_at: Option<Instant>,
    pub last_transition_reason: Option<String>,
    pub last_error: Option<String>,
    pub last_heartbeat_at: Option<Instant>,
}

impl RetryTracker {
    pub const fn new() -> Self {
        Self {
            attempts: 0,
            degraded: false,
            first_failure_at: None,
            next_retry_at: None,
            last_transition_reason: None,
            last_error: None,
            last_heartbeat_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackoffConfig {
    pub initial_ms: u64,
    pub factor: f64,
    pub max_ms: u64,
    pub jitter_percent: u8,
    pub max_window_ms: u64,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            initial_ms: 250,
            factor: 2.0,
            max_ms: 30_000,
            jitter_percent: 20,
            max_window_ms: 600_000,
        }
    }
}

impl BackoffConfig {
    pub fn compute_delay(&self, service: &str, attempts: u32) -> Duration {
        let base = (self.initial_ms as f64 * self.factor.powi((attempts.saturating_sub(1)) as i32))
            .round()
            .clamp(self.initial_ms as f64, self.max_ms as f64) as u64;

        if self.jitter_percent == 0 {
            return Duration::from_millis(base);
        }

        let jitter = deterministic_jitter(service, attempts, self.jitter_percent);
        let adjusted = (base as f64 * (1.0 + jitter)).round().max(1.0) as u64;
        Duration::from_millis(adjusted)
    }
}

fn deterministic_jitter(service: &str, attempts: u32, jitter_percent: u8) -> f64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in service.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash ^= attempts as u64;
    hash = hash.wrapping_mul(0x100000001b3);

    let max = jitter_percent as f64 / 100.0;
    let unit = (hash as f64) / (u64::MAX as f64);
    -max + (2.0 * max * unit)
}

#[cfg(test)]
mod tests {
    use super::BackoffConfig;

    #[test]
    fn backoff_delay_is_bounded() {
        let cfg = BackoffConfig::default();
        let delay = cfg.compute_delay("My/Service", 16);
        assert!(delay.as_millis() <= cfg.max_ms as u128);
        assert!(delay.as_millis() > 0);
    }
}
