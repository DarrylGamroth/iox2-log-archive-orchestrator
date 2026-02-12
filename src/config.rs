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

use std::env;
use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use serde::Deserialize;

use crate::cli::Cli;
use crate::model::BackoffConfig;

#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub state_path: PathBuf,
    pub control_service: String,
    pub recorder_bin: String,
    pub control_bin: String,
    pub control_timeout_ms: u64,
    pub reconcile_interval_ms: u64,
    pub backoff: BackoffConfig,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct FileConfig {
    state_path: Option<PathBuf>,
    control_service: Option<String>,
    recorder_bin: Option<String>,
    control_bin: Option<String>,
    control_timeout_ms: Option<u64>,
    reconcile_interval_ms: Option<u64>,
    backoff_initial_ms: Option<u64>,
    backoff_factor: Option<f64>,
    backoff_max_ms: Option<u64>,
    backoff_jitter_percent: Option<u8>,
    backoff_max_window_ms: Option<u64>,
}

pub fn resolve(cli: &Cli) -> anyhow::Result<ResolvedConfig> {
    let file_cfg = if let Some(path) = &cli.config_path {
        load_file_config(path)?
    } else {
        FileConfig::default()
    };

    let state_path = resolve_path(
        cli.state_path.clone(),
        "IOX2_LOG_ORCH_STATE_PATH",
        file_cfg.state_path,
        default_state_path(),
    );
    let control_service = resolve_string(
        cli.control_service.clone(),
        "IOX2_LOG_ORCH_CONTROL_SERVICE",
        file_cfg.control_service,
        "iox2/log/archive/orchestrator/control".to_string(),
    );

    let recorder_bin = resolve_string(
        cli.recorder_bin.clone(),
        "IOX2_LOG_ORCH_RECORDER_BIN",
        file_cfg.recorder_bin,
        "iox2-log-recorder".to_string(),
    );
    let control_bin = resolve_string(
        cli.control_bin.clone(),
        "IOX2_LOG_ORCH_CONTROL_BIN",
        file_cfg.control_bin,
        "iox2-log-control".to_string(),
    );
    let control_timeout_ms = resolve_u64(
        cli.control_timeout_ms,
        "IOX2_LOG_ORCH_CONTROL_TIMEOUT_MS",
        file_cfg.control_timeout_ms,
        2000,
    )?;
    let reconcile_interval_ms = resolve_u64(
        cli.reconcile_interval_ms,
        "IOX2_LOG_ORCH_RECONCILE_INTERVAL_MS",
        file_cfg.reconcile_interval_ms,
        1000,
    )?;

    let backoff = BackoffConfig {
        initial_ms: resolve_u64(
            cli.backoff_initial_ms,
            "IOX2_LOG_ORCH_BACKOFF_INITIAL_MS",
            file_cfg.backoff_initial_ms,
            250,
        )?,
        factor: resolve_f64(
            cli.backoff_factor,
            "IOX2_LOG_ORCH_BACKOFF_FACTOR",
            file_cfg.backoff_factor,
            2.0,
        )?,
        max_ms: resolve_u64(
            cli.backoff_max_ms,
            "IOX2_LOG_ORCH_BACKOFF_MAX_MS",
            file_cfg.backoff_max_ms,
            30_000,
        )?,
        jitter_percent: resolve_u8(
            cli.backoff_jitter_percent,
            "IOX2_LOG_ORCH_BACKOFF_JITTER_PERCENT",
            file_cfg.backoff_jitter_percent,
            20,
        )?,
        max_window_ms: resolve_u64(
            cli.backoff_max_window_ms,
            "IOX2_LOG_ORCH_BACKOFF_MAX_WINDOW_MS",
            file_cfg.backoff_max_window_ms,
            600_000,
        )?,
    };

    Ok(ResolvedConfig {
        state_path,
        control_service,
        recorder_bin,
        control_bin,
        control_timeout_ms,
        reconcile_interval_ms,
        backoff,
    })
}

fn load_file_config(path: &PathBuf) -> anyhow::Result<FileConfig> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read config file {}", path.display()))?;
    toml::from_str::<FileConfig>(&content)
        .with_context(|| format!("failed to parse config file {}", path.display()))
}

fn resolve_string(
    cli_value: Option<String>,
    env_key: &str,
    file_value: Option<String>,
    default_value: String,
) -> String {
    cli_value
        .or_else(|| env::var(env_key).ok())
        .or(file_value)
        .unwrap_or(default_value)
}

fn resolve_path(
    cli_value: Option<PathBuf>,
    env_key: &str,
    file_value: Option<PathBuf>,
    default_value: PathBuf,
) -> PathBuf {
    cli_value
        .or_else(|| env::var_os(env_key).map(PathBuf::from))
        .or(file_value)
        .unwrap_or(default_value)
}

fn resolve_u64(
    cli_value: Option<u64>,
    env_key: &str,
    file_value: Option<u64>,
    default_value: u64,
) -> anyhow::Result<u64> {
    let parsed_env = env::var(env_key)
        .ok()
        .map(|value| {
            value.parse::<u64>().with_context(|| {
                format!(
                    "failed to parse environment variable {}='{}' as u64",
                    env_key, value
                )
            })
        })
        .transpose()?;
    Ok(cli_value
        .or(parsed_env)
        .or(file_value)
        .unwrap_or(default_value))
}

fn resolve_u8(
    cli_value: Option<u8>,
    env_key: &str,
    file_value: Option<u8>,
    default_value: u8,
) -> anyhow::Result<u8> {
    let parsed_env = env::var(env_key)
        .ok()
        .map(|value| {
            value.parse::<u8>().with_context(|| {
                format!(
                    "failed to parse environment variable {}='{}' as u8",
                    env_key, value
                )
            })
        })
        .transpose()?;
    Ok(cli_value
        .or(parsed_env)
        .or(file_value)
        .unwrap_or(default_value))
}

fn resolve_f64(
    cli_value: Option<f64>,
    env_key: &str,
    file_value: Option<f64>,
    default_value: f64,
) -> anyhow::Result<f64> {
    let parsed_env = env::var(env_key)
        .ok()
        .map(|value| {
            value.parse::<f64>().with_context(|| {
                format!(
                    "failed to parse environment variable {}='{}' as f64",
                    env_key, value
                )
            })
        })
        .transpose()?;
    Ok(cli_value
        .or(parsed_env)
        .or(file_value)
        .unwrap_or(default_value))
}

fn default_state_path() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        return home
            .join(".config")
            .join("iox2")
            .join("log-orchestrator")
            .join("state.toml");
    }
    PathBuf::from("/tmp/iox2-log-orchestrator/state.toml")
}

#[cfg(test)]
mod tests {
    use super::resolve_u64;

    #[test]
    fn cli_overrides_defaults() {
        let value = resolve_u64(Some(7), "DOES_NOT_EXIST", None, 42).unwrap();
        assert_eq!(value, 7);
    }
}
