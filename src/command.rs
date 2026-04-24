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

use std::time::Duration;

use anyhow::Context;

use crate::cli::{Cli, OrchestratorAction};
use crate::client::send_request;
use crate::config::resolve;
use crate::control::ControlClient;
use crate::control_api::{
    EnableRequest, RequestCommand, RequestEnvelope, ResponsePayload, ServiceRequest,
};
use crate::daemon::{Daemon, DaemonConfig};
use crate::error::CommandError;
use crate::model::CONTROL_API_VERSION;
use crate::state::load_or_default;
use crate::worker::ProcessRunner;

pub fn run(cli: Cli) -> Result<(), CommandError> {
    let action = cli.action.as_ref().ok_or_else(|| {
        CommandError::InvalidInput(
            "a command is required (serve/enable/disable/pause/resume/start/stop/status/list/reconcile/daemon-status/shutdown)".to_string(),
        )
    })?;
    let cfg = resolve(&cli).map_err(CommandError::Internal)?;

    if matches!(action, OrchestratorAction::Serve) {
        let state = load_or_default(&cfg.state_path).map_err(CommandError::Internal)?;
        let daemon_cfg = DaemonConfig {
            state_path: cfg.state_path.clone(),
            control_service: cfg.control_service,
            recorder_bin: cfg.recorder_bin,
            control: ControlClient {
                control_bin: cfg.control_bin,
                timeout_ms: cfg.control_timeout_ms,
            },
            reconcile_interval: Duration::from_millis(cfg.reconcile_interval_ms),
            backoff: cfg.backoff,
        };
        return Daemon::new(daemon_cfg, ProcessRunner, state).run();
    }

    let control_service = cfg.control_service.clone();
    let request = request_from_action(action)?;
    let response = send_request(
        &control_service,
        &request,
        Duration::from_millis(cfg.control_timeout_ms),
    )
    .map_err(|error| {
        CommandError::NotAvailable(format!(
            "orchestrator daemon is not reachable on control service '{}': {error:#}",
            control_service
        ))
    })?;

    if response.version != CONTROL_API_VERSION {
        return Err(CommandError::NotAvailable(format!(
            "orchestrator control API version mismatch: daemon={}, client={}",
            response.version, CONTROL_API_VERSION
        )));
    }

    match response.response {
        ResponsePayload::Ok(payload) => print(&payload, cli.format),
        ResponsePayload::Error(error) => Err(match error.exit_code {
            2 => CommandError::InvalidInput(error.message),
            3 => CommandError::NotAvailable(error.message),
            _ => CommandError::Internal(anyhow::anyhow!(error.message)),
        }),
    }
}

fn request_from_action(action: &OrchestratorAction) -> Result<RequestEnvelope, CommandError> {
    let command = match action {
        OrchestratorAction::Serve => {
            return Err(CommandError::InvalidInput(
                "internal error: serve cannot be sent over control API".to_string(),
            ))
        }
        OrchestratorAction::Enable(options) => RequestCommand::Enable(EnableRequest {
            service: options.service.clone(),
            instance: options.instance.clone(),
            storage_path: options.storage_path.clone(),
            metadata_log_path: options.metadata_log_path.clone(),
            profile: options.profile,
            mode: options.mode,
            cycle_time_ms: options.cycle_time_ms,
            flush_interval_ms: options.flush_interval_ms,
            max_disk_bytes: options.max_disk_bytes,
            async_io_backend: options.async_io_backend,
            io_uring_queue_depth: options.io_uring_queue_depth,
            io_submit_batch_max: options.io_submit_batch_max,
            io_cqe_batch_max: options.io_cqe_batch_max,
            io_uring_register_files: options.io_uring_register_files,
            checksum_mode: options.checksum_mode,
            out_of_space_policy: options.out_of_space_policy,
            metadata_log_roll_bytes: options.metadata_log_roll_bytes,
            metadata_log_max_bytes: options.metadata_log_max_bytes,
        }),
        OrchestratorAction::Disable(options) => RequestCommand::Disable(ServiceRequest {
            service: options.service.clone(),
        }),
        OrchestratorAction::Pause(options) => RequestCommand::Pause(ServiceRequest {
            service: options.service.clone(),
        }),
        OrchestratorAction::Resume(options) => RequestCommand::Resume(ServiceRequest {
            service: options.service.clone(),
        }),
        OrchestratorAction::Start(options) => RequestCommand::Start(ServiceRequest {
            service: options.service.clone(),
        }),
        OrchestratorAction::Stop(options) => RequestCommand::Stop(ServiceRequest {
            service: options.service.clone(),
        }),
        OrchestratorAction::Status(options) => RequestCommand::Status(ServiceRequest {
            service: options.service.clone(),
        }),
        OrchestratorAction::List => RequestCommand::List,
        OrchestratorAction::Reconcile => RequestCommand::Reconcile,
        OrchestratorAction::DaemonStatus => RequestCommand::DaemonStatus,
        OrchestratorAction::Shutdown => RequestCommand::Shutdown,
    };

    Ok(RequestEnvelope {
        version: CONTROL_API_VERSION,
        command,
    })
}

fn print(value: &impl serde::Serialize, format: crate::format::Format) -> Result<(), CommandError> {
    let output = format
        .as_string(value)
        .with_context(|| "failed to serialize orchestrator output")
        .map_err(CommandError::Internal)?;
    println!("{output}");
    Ok(())
}
