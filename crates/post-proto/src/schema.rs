use schemars::{Schema, schema_for};

use crate::{ClientMessage, ServerMessage};

#[must_use]
pub fn client_message_schema() -> String {
    render(&schema_for!(ClientMessage))
}

#[must_use]
pub fn server_message_schema() -> String {
    render(&schema_for!(ServerMessage))
}

fn render(schema: &Schema) -> String {
    let mut text = serde_json::to_string_pretty(schema).expect("a schema always has a JSON form");
    text.push('\n');
    text
}
