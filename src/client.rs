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

use std::time::{Duration, Instant};

use anyhow::Context;
use iceoryx2::prelude::*;

use crate::control_api::{RequestEnvelope, ResponseEnvelope};

pub fn send_request(
    control_service_name: &str,
    request: &RequestEnvelope,
    timeout: Duration,
) -> anyhow::Result<ResponseEnvelope> {
    let request_payload =
        serde_json::to_vec(request).context("failed to serialize orchestrator request")?;

    let node = NodeBuilder::new()
        .create::<ipc::Service>()
        .context("failed to create iceoryx2 node for orchestrator client")?;

    let service_name = control_service_name
        .try_into()
        .with_context(|| format!("invalid control service name '{}'", control_service_name))?;

    let service = node
        .service_builder(&service_name)
        .request_response::<[u8], [u8]>()
        .open_or_create()
        .with_context(|| {
            format!(
                "failed to open_or_create control request/response service '{}'",
                control_service_name
            )
        })?;

    let client = service
        .client_builder()
        .initial_max_slice_len(request_payload.len().max(64))
        .allocation_strategy(AllocationStrategy::PowerOfTwo)
        .create()
        .context("failed to create orchestrator control client port")?;

    let request = client
        .loan_slice_uninit(request_payload.len().max(1))
        .context("failed to loan request payload buffer")?;
    let request = request.write_from_slice(if request_payload.is_empty() {
        &[0u8]
    } else {
        &request_payload
    });
    let pending_response = request
        .send()
        .context("failed to send orchestrator control request")?;

    let deadline = Instant::now() + timeout;
    loop {
        if let Some(response) = pending_response
            .receive()
            .context("failed to receive orchestrator control response")?
        {
            let payload = serde_json::from_slice::<ResponseEnvelope>(response.payload())
                .context("failed to deserialize orchestrator response envelope")?;
            return Ok(payload);
        }

        let now = Instant::now();
        if now >= deadline {
            anyhow::bail!(
                "timed out waiting for orchestrator response after {} ms",
                timeout.as_millis()
            );
        }

        let wait_time = (deadline - now).min(Duration::from_millis(20));
        if node.wait(wait_time).is_err() {
            break;
        }
    }

    anyhow::bail!(
        "orchestrator response wait interrupted before timeout ({} ms)",
        timeout.as_millis()
    );
}
