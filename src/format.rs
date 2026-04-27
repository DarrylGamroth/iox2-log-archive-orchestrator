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

use anyhow::Context;
use clap::ValueEnum;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum Format {
    #[default]
    RON,
    JSON,
    YAML,
}

impl Format {
    pub fn as_string(self, value: impl Serialize) -> anyhow::Result<String> {
        match self {
            Format::RON => ron::to_string(&value).context("failed to serialize RON"),
            Format::JSON => {
                serde_json::to_string_pretty(&value).context("failed to serialize JSON")
            }
            Format::YAML => serde_yaml::to_string(&value).context("failed to serialize YAML"),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde::Serialize;

    use super::Format;

    #[derive(Serialize)]
    struct Payload {
        service: &'static str,
        enabled: bool,
    }

    #[test]
    fn all_output_formats_serialize_payloads() {
        let payload = Payload {
            service: "Camera/A",
            enabled: true,
        };

        assert!(Format::RON
            .as_string(&payload)
            .unwrap()
            .contains("Camera/A"));
        assert!(Format::JSON
            .as_string(&payload)
            .unwrap()
            .contains("\"enabled\": true"));
        assert!(Format::YAML
            .as_string(&payload)
            .unwrap()
            .contains("service: Camera/A"));
    }
}
