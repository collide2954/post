#![no_main]

use libfuzzer_sys::fuzz_target;
use post_proto::Timestamp;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(time) = Timestamp::parse(text) {
        assert_eq!(time.to_string(), text);
        assert!(time <= Timestamp::MAX);
    }
});
