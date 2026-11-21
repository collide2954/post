use std::fmt;

use serde::Serialize;
use serde::de::DeserializeOwned;

pub const MAX_LINE_BYTES: usize = 65_536;

#[derive(Debug)]
pub enum DecodeError {
    TooLong,
    LineBreak,
    Invalid(serde_json::Error),
}

pub(crate) fn encode<M: Serialize>(message: &M) -> String {
    serde_json::to_string(message).expect("a protocol message always has a JSON form")
}

pub(crate) fn decode<M: DeserializeOwned>(line: &str) -> Result<M, DecodeError> {
    if line.len() > MAX_LINE_BYTES {
        return Err(DecodeError::TooLong);
    }
    if line.contains(['\n', '\r']) {
        return Err(DecodeError::LineBreak);
    }
    serde_json::from_str(line).map_err(DecodeError::Invalid)
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLong => write!(
                formatter,
                "a line can have a maximum of {MAX_LINE_BYTES} bytes"
            ),
            Self::LineBreak => formatter.write_str("a message must be on one line"),
            Self::Invalid(error) => write!(formatter, "the line is not a valid message: {error}"),
        }
    }
}

impl std::error::Error for DecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Invalid(error) => Some(error),
            Self::TooLong | Self::LineBreak => None,
        }
    }
}
