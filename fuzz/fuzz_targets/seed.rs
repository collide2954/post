#![no_main]

use libfuzzer_sys::fuzz_target;
use post_crypto::Seed;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(seed) = Seed::decode(text) {
        let line = text
            .strip_suffix("\r\n")
            .or_else(|| text.strip_suffix('\n'))
            .unwrap_or(text);
        assert_eq!(seed.encode().as_str(), line);
    }
});
