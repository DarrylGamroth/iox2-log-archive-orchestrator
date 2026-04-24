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

use clap::Args;
use clap::Parser;
use clap::Subcommand;

use crate::format::Format;
use crate::model::{
    AsyncIoBackend, ChecksumMode, OutOfSpacePolicy, PersistenceMode, RecorderProfile,
};

#[derive(Parser)]
#[command(
    name = "iox2-log-orchestrator",
    about = "Orchestrate multi-stream log-recorder workers",
    version = env!("CARGO_PKG_VERSION"),
    disable_help_subcommand = true,
)]
pub struct Cli {
    #[clap(subcommand)]
    pub action: Option<OrchestratorAction>,

    #[clap(long, short = 'f', value_enum, global = true, default_value_t = Format::RON)]
    pub format: Format,

    #[clap(long, global = true, help = "Path to desired-state TOML file")]
    pub state_path: Option<PathBuf>,

    #[clap(long, global = true, help = "Path to orchestrator config TOML file")]
    pub config_path: Option<PathBuf>,

    #[clap(long, global = true, help = "iceoryx2 control service name")]
    pub control_service: Option<String>,

    #[clap(long, global = true)]
    pub recorder_bin: Option<String>,

    #[clap(long, global = true)]
    pub control_bin: Option<String>,

    #[clap(long, global = true)]
    pub control_timeout_ms: Option<u64>,

    #[clap(long, global = true)]
    pub reconcile_interval_ms: Option<u64>,

    #[clap(long, global = true)]
    pub backoff_initial_ms: Option<u64>,

    #[clap(long, global = true)]
    pub backoff_factor: Option<f64>,

    #[clap(long, global = true)]
    pub backoff_max_ms: Option<u64>,

    #[clap(long, global = true)]
    pub backoff_jitter_percent: Option<u8>,

    #[clap(long, global = true)]
    pub backoff_max_window_ms: Option<u64>,
}

#[derive(Subcommand)]
pub enum OrchestratorAction {
    Serve,
    Enable(EnableOptions),
    Disable(ServiceOptions),
    Pause(ServiceOptions),
    Resume(ServiceOptions),
    Start(ServiceOptions),
    Stop(ServiceOptions),
    Status(ServiceOptions),
    List,
    Reconcile,
    DaemonStatus,
    Shutdown,
}

#[derive(Clone, Debug, Args)]
pub struct ServiceOptions {
    #[clap(long)]
    pub service: String,
}

#[derive(Clone, Debug, Args)]
pub struct EnableOptions {
    #[clap(long)]
    pub service: String,

    #[clap(long, default_value = "default")]
    pub instance: String,

    #[clap(long)]
    pub storage_path: String,

    #[clap(long)]
    pub metadata_log_path: String,

    #[clap(long, value_enum, default_value_t = RecorderProfile::Balanced)]
    pub profile: RecorderProfile,

    #[clap(long, value_enum, default_value_t = PersistenceMode::Async)]
    pub mode: PersistenceMode,

    #[clap(long, default_value = "10")]
    pub cycle_time_ms: u64,

    #[clap(long, default_value = "100")]
    pub flush_interval_ms: u64,

    #[clap(long)]
    pub max_disk_bytes: Option<u64>,

    #[clap(long, value_enum)]
    pub async_io_backend: Option<AsyncIoBackend>,

    #[clap(long)]
    pub io_uring_queue_depth: Option<u32>,

    #[clap(long)]
    pub io_submit_batch_max: Option<u32>,

    #[clap(long)]
    pub io_cqe_batch_max: Option<u32>,

    #[clap(long)]
    pub io_uring_register_files: Option<bool>,

    #[clap(long, value_enum)]
    pub checksum_mode: Option<ChecksumMode>,

    #[clap(long, value_enum)]
    pub out_of_space_policy: Option<OutOfSpacePolicy>,

    #[clap(long)]
    pub metadata_log_roll_bytes: Option<u64>,

    #[clap(long)]
    pub metadata_log_max_bytes: Option<u64>,
}
