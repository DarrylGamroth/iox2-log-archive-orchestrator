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

use serde::{Deserialize, Serialize};

use crate::model::{
    AsyncIoBackend, ChecksumMode, OutOfSpacePolicy, PersistenceMode, RecorderProfile, ServiceHealth,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestEnvelope {
    pub version: u16,
    pub command: RequestCommand,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "kebab-case")]
pub enum RequestCommand {
    Enable(EnableRequest),
    Disable(ServiceRequest),
    Pause(ServiceRequest),
    Resume(ServiceRequest),
    Start(ServiceRequest),
    Stop(ServiceRequest),
    Status(ServiceRequest),
    List,
    Reconcile,
    DaemonStatus,
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceRequest {
    pub service: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnableRequest {
    pub service: String,
    pub instance: String,
    pub storage_path: String,
    pub metadata_log_path: String,
    pub profile: RecorderProfile,
    pub mode: PersistenceMode,
    pub cycle_time_ms: u64,
    pub flush_interval_ms: u64,
    pub max_disk_bytes: Option<u64>,
    pub async_io_backend: Option<AsyncIoBackend>,
    pub io_uring_queue_depth: Option<u32>,
    pub io_submit_batch_max: Option<u32>,
    pub io_cqe_batch_max: Option<u32>,
    pub io_uring_register_files: Option<bool>,
    pub checksum_mode: Option<ChecksumMode>,
    pub out_of_space_policy: Option<OutOfSpacePolicy>,
    pub metadata_log_roll_bytes: Option<u64>,
    pub metadata_log_max_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseEnvelope {
    pub version: u16,
    pub response: ResponsePayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ResponsePayload {
    Ok(Box<CommandResponse>),
    Error(ErrorPayload),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub error_code: String,
    pub message: String,
    pub exit_code: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case")]
pub enum CommandResponse {
    Enable {
        service: String,
        changed: bool,
        generation: u64,
        started: bool,
    },
    Disable {
        service: String,
        changed: bool,
        stop_requested: bool,
    },
    Pause {
        service: String,
        changed: bool,
        stop_requested: bool,
    },
    Resume {
        service: String,
        changed: bool,
        started: bool,
    },
    Start {
        service: String,
        changed: bool,
        started: bool,
    },
    Stop {
        service: String,
        stop_requested: bool,
    },
    Status {
        service: String,
        configured: bool,
        enabled: bool,
        paused: bool,
        health: Option<ServiceHealth>,
    },
    List {
        services: Vec<ListEntry>,
    },
    Reconcile {
        started_services: Vec<String>,
        stopped_services: Vec<String>,
        already_running_services: Vec<String>,
        degraded_services: Vec<String>,
    },
    DaemonStatus {
        state_path: String,
        control_service: String,
        reconcile_interval_ms: u64,
        known_services: usize,
    },
    Shutdown {
        accepted: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListEntry {
    pub service: String,
    pub enabled: bool,
    pub paused: bool,
    pub health: Option<ServiceHealth>,
}
