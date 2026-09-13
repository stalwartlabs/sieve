/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only
 */

use std::{fs, path::PathBuf};

use serde::Deserialize;

use crate::{
    output::RunOutput,
    run::{Request, ScriptSource},
    settings::Settings,
};

#[derive(Deserialize)]
struct Sample {
    id: String,
    scripts: Vec<SampleFile>,
    messages: Vec<SampleFile>,
    #[serde(default)]
    settings: serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize)]
struct SampleFile {
    name: String,
    file: String,
}

const NOW: i64 = 1_789_210_800;

fn samples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("site/samples")
}

fn load_request(sample: &Sample) -> Request {
    let dir = samples_dir();
    let read = |file: &str| fs::read_to_string(dir.join(file)).expect("sample file");
    let mut settings = serde_json::to_value(Settings::default()).expect("settings");
    if let Some(object) = settings.as_object_mut() {
        object.extend(sample.settings.clone());
    }
    Request {
        scripts: sample
            .scripts
            .iter()
            .map(|script| ScriptSource {
                name: script.name.clone(),
                source: read(&script.file),
            })
            .collect(),
        message: read(&sample.messages[0].file),
        settings: serde_json::from_value(settings).expect("settings"),
        seen_ids: Vec::new(),
        now: NOW,
    }
}

fn describe(output: &RunOutput) -> String {
    let mut lines = Vec::new();
    for event in &output.events {
        let detail = event
            .detail
            .iter()
            .map(|d| format!("{}={}", d.label, d.value))
            .collect::<Vec<_>>()
            .join(" ");
        lines.push(format!(
            "  [{}#{}] {} {}",
            event.kind, event.message_id, event.summary, detail
        ));
    }
    lines.join("\n")
}

fn samples() -> Vec<Sample> {
    serde_json::from_str(&fs::read_to_string(samples_dir().join("index.json")).expect("index"))
        .expect("index.json")
}

#[test]
fn samples_run_cleanly() {
    for sample in samples() {
        let output = load_request(&sample).run();
        println!("{}:\n{}", sample.id, describe(&output));
        assert!(
            output.diagnostics.is_empty(),
            "{}: {:?}",
            sample.id,
            output.diagnostics
        );
        assert!(output.error.is_none(), "{}: {:?}", sample.id, output.error);
        assert!(!output.events.is_empty(), "{}", sample.id);
    }
}

#[test]
fn welcome_sample_shows_features() {
    let sample = samples()
        .into_iter()
        .find(|sample| sample.id == "welcome")
        .expect("welcome sample");
    let request = load_request(&sample);
    let output = request.run();
    let kinds: Vec<_> = output.events.iter().map(|event| event.kind).collect();
    assert!(kinds.contains(&"vacation"), "{kinds:?}");
    assert!(kinds.contains(&"keep"), "{kinds:?}");
    assert!(output.message_changed);
    let modified = output
        .messages
        .iter()
        .find(|message| message.raw.contains("X-Sieve-Filter"))
        .expect("modified message");
    assert!(modified.raw.contains("Regain control of your inbox"));
    assert!(modified.raw.contains("q3-report.pdf"));
    let reply = output
        .messages
        .iter()
        .find(|message| message.subject.starts_with("Away:"))
        .expect("vacation reply");
    assert_eq!(reply.subject, "Away: Q3 report and team offsite");
    assert!(reply.raw.contains("Hi Alice Martin,"));

    let second = Request {
        seen_ids: output.duplicate_ids.clone(),
        ..request
    }
    .run();
    assert_eq!(
        second
            .events
            .iter()
            .map(|event| event.kind)
            .collect::<Vec<_>>(),
        ["discard"]
    );
}

#[test]
fn compile_errors_have_positions() {
    let request = Request {
        scripts: vec![ScriptSource {
            name: "main".into(),
            source: "require \"fileinto\";\n\nif true {\n  fileinto \"x\" \n}\n".into(),
        }],
        ..Default::default()
    };
    let output = request.compile();
    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(output.diagnostics[0].line, 5);

    let request = Request {
        scripts: vec![ScriptSource {
            name: "main".into(),
            source: "fileinto \"x\";".into(),
        }],
        ..Default::default()
    };
    let output = request.compile();
    assert!(
        output.diagnostics[0]
            .message
            .contains("Undeclared capability")
    );
}

#[test]
fn runtime_errors_keep_the_message() {
    let request = Request {
        scripts: vec![ScriptSource {
            name: "main".into(),
            source: "require \"vnd.stalwart.while\";\nwhile \"true\" { }\n".into(),
        }],
        settings: Settings {
            cpu_limit: 100,
            ..Default::default()
        },
        ..Default::default()
    };
    let output = request.run();
    assert!(output.error.is_some());
    assert!(
        output
            .events
            .iter()
            .any(|event| event.kind == "keep" && event.after_error)
    );
}
