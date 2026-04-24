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
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Context;
use iceoryx2::active_request::ActiveRequest;
use iceoryx2::prelude::*;

use crate::control::ControlClient;
use crate::control_api::{
    CommandResponse, EnableRequest, ListEntry, RequestCommand, RequestEnvelope, ResponseEnvelope,
    ResponsePayload, ServiceRequest,
};
use crate::error::CommandError;
use crate::model::{
    BackoffConfig, DesiredState, RetryTracker, ServiceHealth, ServiceIdentity, ServiceSpec,
    CONTROL_API_VERSION,
};
use crate::state::save;
use crate::worker::Runner;

#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub state_path: PathBuf,
    pub control_service: String,
    pub recorder_bin: String,
    pub control: ControlClient,
    pub reconcile_interval: Duration,
    pub backoff: BackoffConfig,
}

#[derive(Debug, Default, Clone)]
struct ReconcileOutcome {
    started_services: Vec<String>,
    stopped_services: Vec<String>,
    already_running_services: Vec<String>,
    degraded_services: Vec<String>,
}

pub struct Daemon<R: Runner> {
    cfg: DaemonConfig,
    runner: R,
    state: DesiredState,
    retry: BTreeMap<String, RetryTracker>,
    should_shutdown: bool,
}

impl<R: Runner> Daemon<R> {
    pub fn new(cfg: DaemonConfig, runner: R, state: DesiredState) -> Self {
        Self {
            cfg,
            runner,
            state,
            retry: BTreeMap::new(),
            should_shutdown: false,
        }
    }

    pub fn run(&mut self) -> Result<(), CommandError> {
        let node = NodeBuilder::new()
            .create::<ipc::Service>()
            .context("failed to create iceoryx2 node for orchestrator daemon")
            .map_err(CommandError::Internal)?;
        let service_name = self
            .cfg
            .control_service
            .as_str()
            .try_into()
            .with_context(|| {
                format!(
                    "invalid orchestrator control service name '{}'",
                    self.cfg.control_service
                )
            })
            .map_err(CommandError::Internal)?;
        let service = node
            .service_builder(&service_name)
            .request_response::<[u8], [u8]>()
            .open_or_create()
            .with_context(|| {
                format!(
                    "failed to open_or_create control request/response service '{}'",
                    self.cfg.control_service
                )
            })
            .map_err(CommandError::Internal)?;
        let server = service
            .server_builder()
            .initial_max_slice_len(1024)
            .allocation_strategy(AllocationStrategy::PowerOfTwo)
            .create()
            .context("failed to create orchestrator control server port")
            .map_err(CommandError::Internal)?;

        let mut last_reconcile = Instant::now()
            .checked_sub(self.cfg.reconcile_interval)
            .unwrap_or_else(Instant::now);

        while !self.should_shutdown {
            while let Some(active_request) = server
                .receive()
                .context("failed to receive orchestrator control request")
                .map_err(CommandError::Internal)?
            {
                self.handle_request_sample(active_request)?;
            }

            if last_reconcile.elapsed() >= self.cfg.reconcile_interval {
                let _ = self.reconcile_once()?;
                last_reconcile = Instant::now();
            }

            let _ = node.wait(Duration::from_millis(20));
        }

        Ok(())
    }

    fn handle_request_sample(
        &mut self,
        active_request: ActiveRequest<ipc::Service, [u8], (), [u8], ()>,
    ) -> Result<(), CommandError> {
        let request_buf = active_request.payload();
        let request: RequestEnvelope = serde_json::from_slice(request_buf)
            .context("failed to parse orchestrator control request envelope")
            .map_err(CommandError::Internal)?;

        let response = if request.version != CONTROL_API_VERSION {
            ResponseEnvelope {
                version: CONTROL_API_VERSION,
                response: ResponsePayload::Error(
                    CommandError::InvalidInput(format!(
                        "unsupported control API version {}, expected {}",
                        request.version, CONTROL_API_VERSION
                    ))
                    .to_payload(),
                ),
            }
        } else {
            match self.handle_request(request.command) {
                Ok(payload) => ResponseEnvelope {
                    version: CONTROL_API_VERSION,
                    response: ResponsePayload::Ok(Box::new(payload)),
                },
                Err(error) => ResponseEnvelope {
                    version: CONTROL_API_VERSION,
                    response: ResponsePayload::Error(error.to_payload()),
                },
            }
        };

        let encoded = serde_json::to_vec(&response)
            .context("failed to encode orchestrator response envelope")
            .map_err(CommandError::Internal)?;

        let response = active_request
            .loan_slice_uninit(encoded.len().max(1))
            .context("failed to loan response payload buffer")
            .map_err(CommandError::Internal)?;
        let response =
            response.write_from_slice(if encoded.is_empty() { &[0u8] } else { &encoded });
        response
            .send()
            .context("failed to send orchestrator control response")
            .map_err(CommandError::Internal)?;
        Ok(())
    }

    fn handle_request(&mut self, command: RequestCommand) -> Result<CommandResponse, CommandError> {
        match command {
            RequestCommand::Enable(req) => self.enable(req),
            RequestCommand::Disable(req) => self.disable(req),
            RequestCommand::Pause(req) => self.pause(req),
            RequestCommand::Resume(req) => self.resume(req),
            RequestCommand::Start(req) => self.start(req),
            RequestCommand::Stop(req) => self.stop(req),
            RequestCommand::Status(req) => self.status(req),
            RequestCommand::List => self.list(),
            RequestCommand::Reconcile => {
                let outcome = self.reconcile_once()?;
                Ok(CommandResponse::Reconcile {
                    started_services: outcome.started_services,
                    stopped_services: outcome.stopped_services,
                    already_running_services: outcome.already_running_services,
                    degraded_services: outcome.degraded_services,
                })
            }
            RequestCommand::DaemonStatus => Ok(CommandResponse::DaemonStatus {
                state_path: self.cfg.state_path.display().to_string(),
                control_service: self.cfg.control_service.clone(),
                reconcile_interval_ms: self.cfg.reconcile_interval.as_millis() as u64,
                known_services: self.state.services.len(),
            }),
            RequestCommand::Shutdown => {
                self.should_shutdown = true;
                Ok(CommandResponse::Shutdown { accepted: true })
            }
        }
    }

    fn enable(&mut self, req: EnableRequest) -> Result<CommandResponse, CommandError> {
        if req.service.trim().is_empty() {
            return Err(CommandError::InvalidInput(
                "--service must not be empty".to_string(),
            ));
        }

        let existing = self.state.services.get(&req.service).cloned();
        let mut spec = ServiceSpec {
            enabled: true,
            paused: false,
            instance: req.instance,
            generation: 1,
            storage_path: req.storage_path,
            metadata_log_path: req.metadata_log_path,
            profile: req.profile,
            mode: req.mode,
            cycle_time_ms: req.cycle_time_ms,
            flush_interval_ms: req.flush_interval_ms,
            max_disk_bytes: req.max_disk_bytes,
            async_io_backend: req.async_io_backend,
            io_uring_queue_depth: req.io_uring_queue_depth,
            io_submit_batch_max: req.io_submit_batch_max,
            io_cqe_batch_max: req.io_cqe_batch_max,
            io_uring_register_files: req.io_uring_register_files,
            checksum_mode: req.checksum_mode,
            out_of_space_policy: req.out_of_space_policy,
            metadata_log_roll_bytes: req.metadata_log_roll_bytes,
            metadata_log_max_bytes: req.metadata_log_max_bytes,
        };

        let changed = if let Some(old) = existing {
            let mut old_cmp = old.clone();
            old_cmp.enabled = true;
            old_cmp.paused = false;
            spec.generation = old.generation;
            if old_cmp == spec {
                false
            } else {
                spec.generation = old.generation.saturating_add(1).max(1);
                true
            }
        } else {
            true
        };

        spec.validate(&req.service)
            .map_err(|error| CommandError::InvalidInput(error.to_string()))?;
        self.state
            .services
            .insert(req.service.clone(), spec.clone());
        save(&self.cfg.state_path, &self.state).map_err(CommandError::Internal)?;
        self.reset_retry_state(&req.service);

        let outcome = self.reconcile_once()?;
        Ok(CommandResponse::Enable {
            service: req.service.clone(),
            changed,
            generation: spec.generation,
            started: outcome.started_services.contains(&req.service),
        })
    }

    fn disable(&mut self, req: ServiceRequest) -> Result<CommandResponse, CommandError> {
        ensure_service(&req.service)?;
        let changed = if let Some(spec) = self.state.services.get_mut(&req.service) {
            let was_enabled = spec.enabled;
            spec.enabled = false;
            spec.paused = false;
            was_enabled
        } else {
            false
        };
        save(&self.cfg.state_path, &self.state).map_err(CommandError::Internal)?;
        let outcome = self.reconcile_once()?;
        Ok(CommandResponse::Disable {
            service: req.service.clone(),
            changed,
            stop_requested: outcome.stopped_services.contains(&req.service),
        })
    }

    fn pause(&mut self, req: ServiceRequest) -> Result<CommandResponse, CommandError> {
        ensure_service(&req.service)?;
        let spec = self.state.services.get_mut(&req.service).ok_or_else(|| {
            CommandError::InvalidInput(format!("unknown service '{}'", req.service))
        })?;

        let changed = !spec.paused;
        spec.paused = true;
        save(&self.cfg.state_path, &self.state).map_err(CommandError::Internal)?;
        let outcome = self.reconcile_once()?;
        Ok(CommandResponse::Pause {
            service: req.service.clone(),
            changed,
            stop_requested: outcome.stopped_services.contains(&req.service),
        })
    }

    fn resume(&mut self, req: ServiceRequest) -> Result<CommandResponse, CommandError> {
        ensure_service(&req.service)?;
        let spec = self.state.services.get_mut(&req.service).ok_or_else(|| {
            CommandError::InvalidInput(format!("unknown service '{}'", req.service))
        })?;

        let changed = spec.paused;
        spec.paused = false;
        save(&self.cfg.state_path, &self.state).map_err(CommandError::Internal)?;
        self.reset_retry_state(&req.service);
        let outcome = self.reconcile_once()?;
        Ok(CommandResponse::Resume {
            service: req.service.clone(),
            changed,
            started: outcome.started_services.contains(&req.service),
        })
    }

    fn start(&mut self, req: ServiceRequest) -> Result<CommandResponse, CommandError> {
        ensure_service(&req.service)?;
        let spec = self.state.services.get_mut(&req.service).ok_or_else(|| {
            CommandError::InvalidInput(format!("unknown service '{}'", req.service))
        })?;

        let changed = !spec.enabled || spec.paused;
        spec.enabled = true;
        spec.paused = false;
        save(&self.cfg.state_path, &self.state).map_err(CommandError::Internal)?;
        self.reset_retry_state(&req.service);
        let outcome = self.reconcile_once()?;
        Ok(CommandResponse::Start {
            service: req.service.clone(),
            changed,
            started: outcome.started_services.contains(&req.service),
        })
    }

    fn stop(&mut self, req: ServiceRequest) -> Result<CommandResponse, CommandError> {
        ensure_service(&req.service)?;
        let live = self
            .cfg
            .control
            .status(&req.service)
            .map_err(CommandError::Internal)?;
        let stop_requested = if live.available {
            let _ = self
                .cfg
                .control
                .stop(&req.service)
                .map_err(CommandError::Internal)?;
            true
        } else {
            false
        };

        Ok(CommandResponse::Stop {
            service: req.service,
            stop_requested,
        })
    }

    fn status(&mut self, req: ServiceRequest) -> Result<CommandResponse, CommandError> {
        ensure_service(&req.service)?;
        let configured = self.state.services.contains_key(&req.service);
        let enabled = self
            .state
            .services
            .get(&req.service)
            .map(|spec| spec.enabled)
            .unwrap_or(false);
        let paused = self
            .state
            .services
            .get(&req.service)
            .map(|spec| spec.paused)
            .unwrap_or(false);
        let health = self.build_health(&req.service)?;

        Ok(CommandResponse::Status {
            service: req.service,
            configured,
            enabled,
            paused,
            health,
        })
    }

    fn list(&mut self) -> Result<CommandResponse, CommandError> {
        let service_specs: Vec<(String, bool, bool)> = self
            .state
            .services
            .iter()
            .map(|(service, spec)| (service.clone(), spec.enabled, spec.paused))
            .collect();

        let mut services = Vec::with_capacity(service_specs.len());
        for (service, enabled, paused) in service_specs {
            services.push(ListEntry {
                service: service.clone(),
                enabled,
                paused,
                health: self.build_health(&service)?,
            });
        }
        Ok(CommandResponse::List { services })
    }

    fn build_health(&mut self, service: &str) -> Result<Option<ServiceHealth>, CommandError> {
        let spec = match self.state.services.get(service) {
            Some(spec) => spec.clone(),
            None => return Ok(None),
        };
        let now = Instant::now();
        let live = self
            .cfg
            .control
            .status(service)
            .map_err(CommandError::Internal)?;
        let tracker = self.retry.entry(service.to_string()).or_default();

        if live.available {
            tracker.last_heartbeat_at = Some(now);
            tracker.last_error = None;
        } else if let Some(message) = &live.message {
            tracker.last_error = Some(message.clone());
        }

        let heartbeat_age_ms = tracker
            .last_heartbeat_at
            .map(|ts| now.saturating_duration_since(ts).as_millis() as u64);
        let next_retry_in_ms = tracker.next_retry_at.map(|ts| {
            if ts > now {
                ts.duration_since(now).as_millis() as u64
            } else {
                0
            }
        });

        Ok(Some(ServiceHealth {
            identity: ServiceIdentity {
                service: service.to_string(),
                instance: spec.instance.clone(),
                generation: spec.generation,
            },
            live,
            heartbeat_age_ms,
            last_error: tracker.last_error.clone(),
            last_transition_reason: tracker.last_transition_reason.clone(),
            restart_attempts: tracker.attempts,
            degraded: tracker.degraded,
            next_retry_in_ms,
        }))
    }

    fn reconcile_once(&mut self) -> Result<ReconcileOutcome, CommandError> {
        let now = Instant::now();
        let mut outcome = ReconcileOutcome::default();

        for (service, spec) in &self.state.services {
            let live = self
                .cfg
                .control
                .status(service)
                .map_err(CommandError::Internal)?;
            let tracker = self.retry.entry(service.clone()).or_default();

            if live.available {
                tracker.last_heartbeat_at = Some(now);
                tracker.last_error = None;
                if tracker.attempts > 0 || tracker.degraded {
                    tracker.attempts = 0;
                    tracker.degraded = false;
                    tracker.first_failure_at = None;
                    tracker.next_retry_at = None;
                    tracker.last_transition_reason = Some("service_recovered".to_string());
                }
            } else if let Some(message) = &live.message {
                tracker.last_error = Some(message.clone());
            }

            if spec.enabled && !spec.paused {
                if live.available {
                    outcome.already_running_services.push(service.clone());
                    continue;
                }

                if tracker.degraded {
                    outcome.degraded_services.push(service.clone());
                    continue;
                }

                if let Some(next_retry) = tracker.next_retry_at {
                    if now < next_retry {
                        continue;
                    }
                }

                match self
                    .runner
                    .spawn_recorder(&self.cfg.recorder_bin, service, spec)
                {
                    Ok(_) => {
                        tracker.attempts = tracker.attempts.saturating_add(1);
                        let first_failure = tracker.first_failure_at.get_or_insert(now);
                        let in_window = now.saturating_duration_since(*first_failure)
                            <= Duration::from_millis(self.cfg.backoff.max_window_ms);

                        if in_window {
                            let delay = self
                                .cfg
                                .backoff
                                .compute_delay(service, tracker.attempts.max(1));
                            tracker.next_retry_at = Some(now + delay);
                            tracker.last_transition_reason =
                                Some("spawned_by_reconcile".to_string());
                            outcome.started_services.push(service.clone());
                        } else {
                            tracker.degraded = true;
                            tracker.last_transition_reason =
                                Some("spawn_retry_window_exhausted".to_string());
                            outcome.degraded_services.push(service.clone());
                        }
                    }
                    Err(error) => {
                        tracker.attempts = tracker.attempts.saturating_add(1);
                        let first_failure = tracker.first_failure_at.get_or_insert(now);
                        let in_window = now.saturating_duration_since(*first_failure)
                            <= Duration::from_millis(self.cfg.backoff.max_window_ms);

                        if in_window {
                            let delay = self
                                .cfg
                                .backoff
                                .compute_delay(service, tracker.attempts.max(1));
                            tracker.next_retry_at = Some(now + delay);
                            tracker.last_transition_reason =
                                Some("spawn_failed_retry_scheduled".to_string());
                        } else {
                            tracker.degraded = true;
                            tracker.last_transition_reason =
                                Some("spawn_failed_degraded".to_string());
                            outcome.degraded_services.push(service.clone());
                        }
                        tracker.last_error = Some(format!("{error:#}"));
                    }
                }
            } else if live.available {
                let _ = self
                    .cfg
                    .control
                    .stop(service)
                    .map_err(CommandError::Internal)?;
                tracker.last_transition_reason = Some(if spec.paused {
                    "stopped_paused".to_string()
                } else {
                    "stopped_disabled".to_string()
                });
                outcome.stopped_services.push(service.clone());
            }
        }

        Ok(outcome)
    }

    fn reset_retry_state(&mut self, service: &str) {
        if let Some(tracker) = self.retry.get_mut(service) {
            tracker.attempts = 0;
            tracker.degraded = false;
            tracker.first_failure_at = None;
            tracker.next_retry_at = None;
            tracker.last_error = None;
            tracker.last_transition_reason = Some("retry_state_reset_by_command".to_string());
        }
    }
}

fn ensure_service(service: &str) -> Result<(), CommandError> {
    if service.trim().is_empty() {
        return Err(CommandError::InvalidInput(
            "--service must not be empty".to_string(),
        ));
    }
    Ok(())
}
