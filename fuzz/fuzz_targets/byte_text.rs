#![no_main]

use libfuzzer_sys::fuzz_target;
use post_proto::{Key, Nonce, Signature};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(key) = Key::parse(text) {
        assert_eq!(key.to_string(), text);
    }
    if let Ok(nonce) = Nonce::parse(text) {
        assert_eq!(nonce.to_string(), text);
    }
    if let Ok(signature) = Signature::parse(text) {
        assert_eq!(signature.to_string(), text);
    }
});
