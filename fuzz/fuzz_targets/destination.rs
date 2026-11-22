#![no_main]

use libfuzzer_sys::fuzz_target;
use post_client::{Destination, Security};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(destination) = Destination::parse(text) {
        let is_local = destination.address().is_local();
        assert!(destination.security() == Security::Tls || is_local);
        let again = Destination::parse(&destination.to_string()).expect("a valid text");
        assert_eq!(again.address(), destination.address());
        assert_eq!(again.security(), destination.security());
        assert_eq!(again.invitation(), None);
    }
});
