#![no_main]

use libfuzzer_sys::fuzz_target;
use post_proto::Address;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(address) = Address::parse(text, 443) {
        let shown = address.to_string();
        assert_eq!(Address::parse(&shown, 1), Ok(address));
    }
});
