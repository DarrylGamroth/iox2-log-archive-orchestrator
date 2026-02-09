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
use crate::model::{PersistenceMode, RecorderProfile};

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

    #[clap(long, global = true, default_value = "iox2-log-recorder")]
    pub recorder_bin: String,

    #[clap(long, global = true, default_value = "iox2-log-control")]
    pub control_bin: String,

    #[clap(long, global = true, default_value = "2000")]
    pub control_timeout_ms: u64,
}

#[derive(Subcommand)]
pub enum OrchestratorAction {
    Enable(EnableOptions),
    Disable(ServiceOptions),
    Status(ServiceOptions),
    List,
    Reconcile,
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
}
