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

use std::path::PathBuf;

use anyhow::Context;
use serde::Serialize;

use crate::cli::{Cli, EnableOptions, OrchestratorAction};
use crate::control::ControlClient;
use crate::format::Format;
use crate::model::{DesiredState, ServiceLiveStatus, ServiceSpec};
use crate::state::{load_or_default, save};
use crate::worker::spawn_recorder;

#[derive(Debug)]
pub enum CommandError {
    InvalidInput(String),
    NotAvailable(String),
    Internal(anyhow::Error),
}

impl CommandError {
    pub const fn exit_code(&self) -> i32 {
        match self {
            Self::Internal(_) => 1,
            Self::InvalidInput(_) => 2,
            Self::NotAvailable(_) => 3,
        }
    }

    pub fn to_formatted_error(&self, format: Format) -> String {
        #[derive(Serialize)]
        struct ErrorPayload<'a> {
            error_code: &'a str,
            message: &'a str,
        }

        let payload = match self {
            Self::InvalidInput(message) => ErrorPayload {
                error_code: "InvalidInput",
                message,
            },
            Self::NotAvailable(message) => ErrorPayload {
                error_code: "NotAvailable",
                message,
            },
            Self::Internal(error) => ErrorPayload {
                error_code: "Internal",
                message: &format!("{error:#}"),
            },
        };

        format
            .as_string(&payload)
            .unwrap_or_else(|_| format!("{}", payload.error_code))
    }
}

#[derive(Serialize)]
struct EnableResult {
    operation: &'static str,
    service: String,
    changed: bool,
    launched_pid: Option<u32>,
}

#[derive(Serialize)]
struct DisableResult {
    operation: &'static str,
    service: String,
    changed: bool,
    stop_requested: bool,
}

#[derive(Serialize)]
struct StatusResult {
    operation: &'static str,
    service: String,
    configured: bool,
    enabled: bool,
    live: ServiceLiveStatus,
}

#[derive(Serialize)]
struct ListEntry {
    service: String,
    enabled: bool,
    live: ServiceLiveStatus,
}

#[derive(Serialize)]
struct ListResult {
    operation: &'static str,
    services: Vec<ListEntry>,
}

#[derive(Serialize)]
struct ReconcileResult {
    operation: &'static str,
    started_services: Vec<String>,
    stopped_services: Vec<String>,
    already_running_services: Vec<String>,
}

pub fn run(cli: Cli) -> Result<(), CommandError> {
    let action = match cli.action {
        Some(action) => action,
        None => {
            return Err(CommandError::InvalidInput(
                "a command is required (enable/disable/status/list/reconcile)".to_string(),
            ))
        }
    };

    let state_path = resolve_state_path(cli.state_path);
    let control = ControlClient {
        control_bin: cli.control_bin,
        timeout_ms: cli.control_timeout_ms,
    };

    match action {
        OrchestratorAction::Enable(options) => enable(&state_path, &cli.recorder_bin, &control, options, cli.format),
        OrchestratorAction::Disable(options) => disable(&state_path, &control, &options.service, cli.format),
        OrchestratorAction::Status(options) => status(&state_path, &control, &options.service, cli.format),
        OrchestratorAction::List => list(&state_path, &control, cli.format),
        OrchestratorAction::Reconcile => reconcile(&state_path, &cli.recorder_bin, &control, cli.format),
    }
}

fn enable(
    state_path: &PathBuf,
    recorder_bin: &str,
    control: &ControlClient,
    options: EnableOptions,
    format: Format,
) -> Result<(), CommandError> {
    let mut state = load_state(state_path)?;

    let spec = ServiceSpec {
        enabled: true,
        storage_path: options.storage_path,
        metadata_log_path: options.metadata_log_path,
        profile: options.profile,
        mode: options.mode,
        cycle_time_ms: options.cycle_time_ms,
        flush_interval_ms: options.flush_interval_ms,
    };
    spec.validate(&options.service)
        .map_err(|e| CommandError::InvalidInput(e.to_string()))?;

    let changed = state.services.get(&options.service) != Some(&spec);
    state.services.insert(options.service.clone(), spec.clone());
    save_state(state_path, &state)?;

    let live = control
        .status(&options.service)
        .map_err(CommandError::Internal)?;

    let launched_pid = if live.available {
        None
    } else {
        Some(
            spawn_recorder(recorder_bin, &options.service, &spec)
                .map_err(CommandError::Internal)?,
        )
    };

    print(
        &EnableResult {
            operation: "enable",
            service: options.service,
            changed,
            launched_pid,
        },
        format,
    )
}

fn disable(
    state_path: &PathBuf,
    control: &ControlClient,
    service: &str,
    format: Format,
) -> Result<(), CommandError> {
    if service.trim().is_empty() {
        return Err(CommandError::InvalidInput(
            "--service must not be empty".to_string(),
        ));
    }

    let mut state = load_state(state_path)?;
    let changed = if let Some(spec) = state.services.get_mut(service) {
        let was_enabled = spec.enabled;
        spec.enabled = false;
        was_enabled
    } else {
        false
    };
    save_state(state_path, &state)?;

    let live = control.status(service).map_err(CommandError::Internal)?;
    let stop_requested = if live.available {
        let _ = control.stop(service).map_err(CommandError::Internal)?;
        true
    } else {
        false
    };

    print(
        &DisableResult {
            operation: "disable",
            service: service.to_string(),
            changed,
            stop_requested,
        },
        format,
    )
}

fn status(
    state_path: &PathBuf,
    control: &ControlClient,
    service: &str,
    format: Format,
) -> Result<(), CommandError> {
    if service.trim().is_empty() {
        return Err(CommandError::InvalidInput(
            "--service must not be empty".to_string(),
        ));
    }

    let state = load_state(state_path)?;
    let configured = state.services.contains_key(service);
    let enabled = state.services.get(service).map(|s| s.enabled).unwrap_or(false);
    let live = control.status(service).map_err(CommandError::Internal)?;

    print(
        &StatusResult {
            operation: "status",
            service: service.to_string(),
            configured,
            enabled,
            live,
        },
        format,
    )
}

fn list(state_path: &PathBuf, control: &ControlClient, format: Format) -> Result<(), CommandError> {
    let state = load_state(state_path)?;
    let mut entries = Vec::with_capacity(state.services.len());

    for (service, spec) in state.services {
        let live = control.status(&service).map_err(CommandError::Internal)?;
        entries.push(ListEntry {
            service,
            enabled: spec.enabled,
            live,
        });
    }

    print(
        &ListResult {
            operation: "list",
            services: entries,
        },
        format,
    )
}

fn reconcile(
    state_path: &PathBuf,
    recorder_bin: &str,
    control: &ControlClient,
    format: Format,
) -> Result<(), CommandError> {
    let state = load_state(state_path)?;

    let mut started_services = Vec::new();
    let mut stopped_services = Vec::new();
    let mut already_running_services = Vec::new();

    for (service, spec) in state.services {
        let live = control.status(&service).map_err(CommandError::Internal)?;

        if spec.enabled {
            if live.available {
                already_running_services.push(service);
            } else {
                spawn_recorder(recorder_bin, &service, &spec).map_err(CommandError::Internal)?;
                started_services.push(service);
            }
        } else if live.available {
            let _ = control.stop(&service).map_err(CommandError::Internal)?;
            stopped_services.push(service);
        }
    }

    print(
        &ReconcileResult {
            operation: "reconcile",
            started_services,
            stopped_services,
            already_running_services,
        },
        format,
    )
}

fn resolve_state_path(path: Option<PathBuf>) -> PathBuf {
    if let Some(path) = path {
        return path;
    }

    if let Some(home) = dirs::home_dir() {
        return home
            .join(".config")
            .join("iox2")
            .join("log-orchestrator")
            .join("state.toml");
    }

    PathBuf::from("/tmp/iox2-log-orchestrator/state.toml")
}

fn load_state(path: &PathBuf) -> Result<DesiredState, CommandError> {
    load_or_default(path).map_err(CommandError::Internal)
}

fn save_state(path: &PathBuf, state: &DesiredState) -> Result<(), CommandError> {
    save(path, state).map_err(CommandError::Internal)
}

fn print(value: &impl Serialize, format: Format) -> Result<(), CommandError> {
    let output = format
        .as_string(value)
        .with_context(|| "failed to serialize orchestrator output")
        .map_err(CommandError::Internal)?;
    println!("{output}");
    Ok(())
}
