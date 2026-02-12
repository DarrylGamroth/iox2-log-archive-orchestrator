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

use serde::Serialize;

use crate::control_api::ErrorPayload;
use crate::format::Format;

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

    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput(_) => "InvalidInput",
            Self::NotAvailable(_) => "NotAvailable",
            Self::Internal(_) => "Internal",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::InvalidInput(message) | Self::NotAvailable(message) => message.clone(),
            Self::Internal(error) => format!("{error:#}"),
        }
    }

    pub fn to_payload(&self) -> ErrorPayload {
        ErrorPayload {
            error_code: self.code().to_string(),
            message: self.message(),
            exit_code: self.exit_code(),
        }
    }

    pub fn to_formatted_error(&self, format: Format) -> String {
        #[derive(Serialize)]
        struct Error<'a> {
            error_code: &'a str,
            message: &'a str,
        }

        let message = self.message();
        let payload = Error {
            error_code: self.code(),
            message: &message,
        };

        format
            .as_string(&payload)
            .unwrap_or_else(|_| self.code().to_string())
    }
}
