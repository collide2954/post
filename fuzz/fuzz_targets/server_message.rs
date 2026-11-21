#![no_main]

use libfuzzer_sys::fuzz_target;
use post_proto::ServerMessage;

fuzz_target!(|data: &[u8]| {
    let Ok(line) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(message) = ServerMessage::decode(line) {
        let again = ServerMessage::decode(&message.encode());
        assert_eq!(again.ok(), Some(message));
    }
});
