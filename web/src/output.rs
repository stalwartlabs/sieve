/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only
 */

use mail_parser::{Addr, Address, MessageParser, MimeHeaders};
use serde::Serialize;
use sieve::{compiler::CompileError, runtime::RuntimeError};

use crate::settings::KeyValue;

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileOutput {
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunOutput {
    pub diagnostics: Vec<Diagnostic>,
    pub error: Option<Diagnostic>,
    pub events: Vec<Event>,
    pub messages: Vec<OutputMessage>,
    pub message_changed: bool,
    pub instructions: usize,
    pub global_variables: Vec<KeyValue>,
    pub duplicate_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub script: usize,
    pub severity: Severity,
    pub line: usize,
    pub column: usize,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub kind: &'static str,
    pub summary: String,
    pub detail: Vec<Detail>,
    pub message_id: usize,
    pub is_final: bool,
    pub after_error: bool,
}

#[derive(Debug, Serialize)]
pub struct Detail {
    pub label: &'static str,
    pub value: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputMessage {
    pub id: usize,
    pub size: usize,
    pub raw: String,
    pub subject: String,
    pub from: String,
    pub to: String,
    pub date: String,
    pub headers: Vec<KeyValue>,
    pub text: Option<String>,
    pub html: Option<String>,
    pub attachments: Vec<Attachment>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub name: String,
    pub content_type: String,
    pub size: usize,
}

const POSITION_SUFFIX: &str = " at line ";

impl Diagnostic {
    pub fn compile(script: usize, err: &CompileError) -> Self {
        let mut message = err.to_string();
        if let Some(pos) = message.rfind(POSITION_SUFFIX) {
            message.truncate(pos);
        }
        Diagnostic {
            script,
            severity: Severity::Error,
            line: err.line_num(),
            column: err.line_pos(),
            message,
        }
    }

    pub fn runtime(err: &RuntimeError) -> Self {
        let (line, column, message) = match err {
            RuntimeError::InvalidInstruction {
                name,
                line_num,
                line_pos,
            } => (
                *line_num as usize,
                *line_pos as usize,
                format!("Unknown or invalid instruction {name:?}"),
            ),
            err => (0, 0, err.to_string()),
        };
        Diagnostic {
            script: 0,
            severity: Severity::Warning,
            line,
            column,
            message,
        }
    }
}

impl Detail {
    pub fn new(label: &'static str, value: impl Into<String>) -> Self {
        Detail {
            label,
            value: value.into(),
        }
    }
}

impl OutputMessage {
    pub fn parse(id: usize, bytes: &[u8]) -> Self {
        let raw = String::from_utf8_lossy(bytes).into_owned();
        let Some(message) = MessageParser::new().parse(bytes) else {
            return OutputMessage {
                id,
                size: bytes.len(),
                raw,
                subject: String::new(),
                from: String::new(),
                to: String::new(),
                date: String::new(),
                headers: Vec::new(),
                text: None,
                html: None,
                attachments: Vec::new(),
            };
        };

        let raw_bytes = message.raw_message();
        let headers = message
            .headers()
            .iter()
            .map(|header| KeyValue {
                name: header.name.as_str().to_string(),
                value: raw_bytes
                    .get(header.offset_start as usize..header.offset_end as usize)
                    .map(|value| {
                        String::from_utf8_lossy(value)
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .unwrap_or_default(),
            })
            .collect();

        let html = message
            .html_body
            .iter()
            .any(|part| {
                message
                    .part(*part)
                    .is_some_and(|part| part.is_content_type("text", "html"))
            })
            .then(|| message.body_html(0).map(|html| html.into_owned()))
            .flatten();

        OutputMessage {
            id,
            size: bytes.len(),
            subject: message.subject().unwrap_or_default().to_string(),
            from: message.from().map(format_address).unwrap_or_default(),
            to: message.to().map(format_address).unwrap_or_default(),
            date: message
                .date()
                .map(|date| date.to_rfc822())
                .unwrap_or_default(),
            headers,
            text: message.body_text(0).map(|text| text.into_owned()),
            html,
            attachments: message
                .attachments()
                .map(|part| Attachment {
                    name: part.attachment_name().unwrap_or("unnamed").to_string(),
                    content_type: part
                        .content_type()
                        .map(|ct| match ct.subtype() {
                            Some(subtype) => format!("{}/{subtype}", ct.ctype()),
                            None => ct.ctype().to_string(),
                        })
                        .unwrap_or_else(|| "application/octet-stream".to_string()),
                    size: part.len(),
                })
                .collect(),
            raw,
        }
    }
}

fn format_address(address: &Address<'_>) -> String {
    address
        .iter()
        .map(|Addr { name, address }| match (name, address) {
            (Some(name), Some(address)) => format!("{name} <{address}>"),
            (None, Some(address)) => address.to_string(),
            (Some(name), None) => name.to_string(),
            (None, None) => String::new(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}
