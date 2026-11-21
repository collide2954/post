use post_proto::DEFAULT_PORT;

pub const USAGE: &str = "usage: post-server [PORT]";

pub fn port_from(arguments: impl IntoIterator<Item = String>) -> Result<u16, &'static str> {
    let mut arguments = arguments.into_iter();
    let port = match arguments.next() {
        None => DEFAULT_PORT,
        Some(text) => text.parse().ok().filter(|port| *port != 0).ok_or(USAGE)?,
    };
    match arguments.next() {
        None => Ok(port),
        Some(_) => Err(USAGE),
    }
}

#[cfg(test)]
mod tests {
    use super::{USAGE, port_from};

    fn arguments(list: &[&str]) -> Vec<String> {
        list.iter().map(|text| (*text).to_owned()).collect()
    }

    #[test]
    fn uses_the_default_port_without_an_argument() {
        assert_eq!(port_from(arguments(&[])), Ok(7678));
    }

    #[test]
    fn reads_a_port_number() {
        assert_eq!(port_from(arguments(&["1"])), Ok(1));
        assert_eq!(port_from(arguments(&["9000"])), Ok(9000));
        assert_eq!(port_from(arguments(&["65535"])), Ok(65535));
    }

    #[test]
    fn refuses_an_argument_that_is_not_a_port_number() {
        for invalid in ["0", "65536", "-1", "port", "", "80 ", "--help"] {
            assert_eq!(port_from(arguments(&[invalid])), Err(USAGE), "{invalid:?}");
        }
    }

    #[test]
    fn refuses_more_than_one_argument() {
        assert_eq!(port_from(arguments(&["9000", "9001"])), Err(USAGE));
    }

    #[test]
    fn tells_how_to_start_the_program() {
        assert_eq!(USAGE, "usage: post-server [PORT]");
    }
}
