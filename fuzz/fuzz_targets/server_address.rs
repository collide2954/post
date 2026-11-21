#![no_main]

use libfuzzer_sys::fuzz_target;
use post_client::ServerAddress;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(address) = ServerAddress::parse(text) {
        assert_eq!(ServerAddress::parse(&address.to_string()).ok(), Some(address.clone()));
        assert!(address
            .candidates()
            .iter()
            .all(|candidate| candidate.ip().is_loopback()));
    }
});
