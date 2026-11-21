mod name;
mod text;
mod time;

pub use name::{Name, NameError};
pub use text::{MAX_TEXT_CHARACTERS, TextError, check_text};
pub use time::{Timestamp, TimestampError};
