use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use post_proto::{ClientMessage, ServerMessage, client_message_schema, server_message_schema};
use serde_json::Value;

const UPDATE_VARIABLE: &str = "POST_UPDATE_SPECIFICATION";

fn specification() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/protocol")
}

fn assert_schema_file(file: &str, schema: &str) {
    let path = specification().join("schemas").join(file);
    if std::env::var_os(UPDATE_VARIABLE).is_some() {
        fs::create_dir_all(path.parent().expect("a schema file has a directory"))
            .expect("cannot make the schema directory");
        fs::write(&path, schema).expect("cannot write the schema file");
        return;
    }
    let written = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    assert_eq!(
        written.replace("\r\n", "\n"),
        schema,
        "{file} does not agree with the code; set {UPDATE_VARIABLE}=1 and run the tests to write it again"
    );
}

fn message_types(schema: &str) -> BTreeSet<String> {
    let schema: Value = serde_json::from_str(schema).expect("a schema is JSON");
    schema["oneOf"]
        .as_array()
        .expect("a message schema has one entry for each message type")
        .iter()
        .map(|entry| {
            entry["properties"]["type"]["const"]
                .as_str()
                .expect("each message type has a name")
                .to_owned()
        })
        .collect()
}

fn error_codes(schema: &str) -> BTreeSet<String> {
    let schema: Value = serde_json::from_str(schema).expect("a schema is JSON");
    schema["$defs"]["ErrorCode"]["oneOf"]
        .as_array()
        .expect("the schema has one entry for each error code")
        .iter()
        .map(|entry| {
            entry["const"]
                .as_str()
                .expect("each error code is a word")
                .to_owned()
        })
        .collect()
}

struct Examples {
    client: Vec<ClientMessage>,
    server: Vec<ServerMessage>,
}

fn read_examples() -> Examples {
    let directory = specification().join("examples");
    let mut examples = Examples {
        client: Vec::new(),
        server: Vec::new(),
    };
    let entries = fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", directory.display()));
    for entry in entries {
        let path = entry.expect("cannot read a directory entry").path();
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        for (index, line) in text.lines().enumerate() {
            let place = format!("{}:{}", path.display(), index + 1);
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(json) = line.strip_prefix("C: ") {
                let message =
                    ClientMessage::decode(json).unwrap_or_else(|error| panic!("{place}: {error}"));
                assert_eq!(message.encode(), json, "{place}");
                examples.client.push(message);
            } else if let Some(json) = line.strip_prefix("S: ") {
                let message =
                    ServerMessage::decode(json).unwrap_or_else(|error| panic!("{place}: {error}"));
                assert_eq!(message.encode(), json, "{place}");
                examples.server.push(message);
            } else {
                panic!("{place}: a line must start with `C: `, `S: `, or `#`");
            }
        }
    }
    examples
}

fn type_of(line: &str) -> String {
    let value: Value = serde_json::from_str(line).expect("a message is JSON");
    value["type"]
        .as_str()
        .expect("a message has a type")
        .to_owned()
}

#[test]
fn the_client_schema_file_agrees_with_the_code() {
    assert_schema_file("client-message.schema.json", &client_message_schema());
}

#[test]
fn the_server_schema_file_agrees_with_the_code() {
    assert_schema_file("server-message.schema.json", &server_message_schema());
}

#[test]
fn the_schemas_list_each_message_type() {
    let client: Vec<String> = message_types(&client_message_schema())
        .into_iter()
        .collect();
    assert_eq!(client, ["guest", "send"]);

    let server: Vec<String> = message_types(&server_message_schema())
        .into_iter()
        .collect();
    assert_eq!(server, ["error", "join", "leave", "message", "welcome"]);
}

#[test]
fn the_examples_show_each_message_type() {
    let examples = read_examples();

    let client: BTreeSet<String> = examples
        .client
        .iter()
        .map(|message| type_of(&message.encode()))
        .collect();
    assert_eq!(client, message_types(&client_message_schema()));

    let server: BTreeSet<String> = examples
        .server
        .iter()
        .map(|message| type_of(&message.encode()))
        .collect();
    assert_eq!(server, message_types(&server_message_schema()));
}

#[test]
fn the_examples_show_each_error_code() {
    let shown: BTreeSet<String> = read_examples()
        .server
        .iter()
        .filter_map(|message| match message {
            ServerMessage::Error { code, text, .. } => {
                assert_eq!(text, code.text());
                let value = serde_json::to_value(code).expect("an error code is JSON");
                Some(value.as_str().expect("an error code is a word").to_owned())
            }
            _ => None,
        })
        .collect();
    assert_eq!(shown, error_codes(&server_message_schema()));
}
