#![no_main]

use libfuzzer_sys::fuzz_target;
use post_proto::Name;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(name) = Name::parse(text) {
        assert_eq!(name.as_str(), text);
        assert!((Name::MIN_LENGTH..=Name::MAX_LENGTH).contains(&text.len()));
    }
});
