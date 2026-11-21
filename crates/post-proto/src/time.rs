use std::borrow::Cow;
use std::fmt::{self, Write as _};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::string::StringSliceAdditionalSpecFns;

use text::{LAST_MILLIS, read_text, write_text};
#[cfg(verus_keep_ghost)]
use text::{last_millis, lemma_the_text_of_each_time, time_text};

mod date;
mod text;

verus! {

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimestampError;

impl Timestamp {
    pub exec const MAX: Self = Self(LAST_MILLIS);

    #[verifier::type_invariant]
    spec fn is_in_range(self) -> bool {
        self.0 <= last_millis()
    }

    pub closed spec fn millis(self) -> int {
        self.0 as int
    }

    #[must_use]
    pub fn from_unix_millis(millis: u64) -> (time: Option<Self>)
        ensures
            time is Some <==> millis <= last_millis(),
            time matches Some(time) ==> time.millis() == millis,
    {
        if millis <= LAST_MILLIS {
            Some(Self(millis))
        } else {
            None
        }
    }

    #[must_use]
    pub fn unix_millis(self) -> (millis: u64)
        ensures
            millis == self.millis(),
            millis <= last_millis(),
    {
        proof {
            use_type_invariant(self);
        }
        self.0
    }

    pub fn parse(text: &str) -> (time: Result<Self, TimestampError>)
        ensures
            forall|millis: int|
                0 <= millis <= last_millis() ==> (text.spec_bytes() == #[trigger] time_text(millis)
                    <==> (time matches Ok(time) && time.millis() == millis)),
    {
        proof {
            lemma_the_text_of_each_time(text.spec_bytes());
        }
        match read_text(text.as_bytes()) {
            Some(millis) => Ok(Self(millis)),
            None => Err(TimestampError),
        }
    }

    fn text(self) -> (text: [u8; 24])
        ensures
            text@ == time_text(self.millis()),
    {
        proof {
            use_type_invariant(self);
        }
        write_text(self.0)
    }
}

} // verus!

impl Timestamp {
    #[must_use]
    pub fn now() -> Self {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_millis());
        u64::try_from(millis)
            .ok()
            .and_then(Self::from_unix_millis)
            .unwrap_or(Self::MAX)
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.text()
            .into_iter()
            .try_for_each(|byte| formatter.write_char(char::from(byte)))
    }
}
impl FromStr for Timestamp {
    type Err = TimestampError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}

impl fmt::Display for TimestampError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a time must have the form 2026-11-21T14:02:07.120Z")
    }
}

impl std::error::Error for TimestampError {}

impl JsonSchema for Timestamp {
    fn schema_name() -> Cow<'static, str> {
        "Timestamp".into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "A time in UTC with milliseconds, for example 2026-11-21T14:02:07.120Z.",
            "type": "string",
            "format": "date-time",
            "pattern": "^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}\\.[0-9]{3}Z$"
        })
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::{Timestamp, TimestampError};

    const KNOWN: [(u64, &str); 12] = [
        (0, "1970-01-01T00:00:00.000Z"),
        (1, "1970-01-01T00:00:00.001Z"),
        (31_535_999_999, "1970-12-31T23:59:59.999Z"),
        (31_536_000_000, "1971-01-01T00:00:00.000Z"),
        (68_169_600_000, "1972-02-29T00:00:00.000Z"),
        (951_782_400_000, "2000-02-29T00:00:00.000Z"),
        (951_868_800_000, "2000-03-01T00:00:00.000Z"),
        (1_000_000_000_000, "2001-09-09T01:46:40.000Z"),
        (1_700_000_000_000, "2023-11-14T22:13:20.000Z"),
        (1_795_269_727_120, "2026-11-21T14:02:07.120Z"),
        (4_107_542_400_000, "2100-03-01T00:00:00.000Z"),
        (253_402_300_799_999, "9999-12-31T23:59:59.999Z"),
    ];

    #[test]
    fn writes_known_times() {
        for (millis, text) in KNOWN {
            let time = Timestamp::from_unix_millis(millis).expect(text);
            assert_eq!(time.to_string(), text);
            assert_eq!(time.unix_millis(), millis);
        }
    }

    #[test]
    fn reads_known_times() {
        for (millis, text) in KNOWN {
            assert_eq!(
                Timestamp::parse(text).map(Timestamp::unix_millis),
                Ok(millis),
                "{text}"
            );
        }
    }

    #[test]
    fn reads_each_time_that_it_writes() {
        let mut millis: u64 = 7;
        for _ in 0..20_000 {
            millis = millis
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let time = Timestamp::from_unix_millis(millis % (Timestamp::MAX.unix_millis() + 1))
                .expect("in range");
            assert_eq!(Timestamp::parse(&time.to_string()), Ok(time));
        }
    }

    #[test]
    fn has_a_maximum_at_the_end_of_the_year_9999() {
        assert_eq!(Timestamp::MAX.unix_millis(), 253_402_300_799_999);
        assert_eq!(
            Timestamp::from_unix_millis(253_402_300_799_999),
            Some(Timestamp::MAX)
        );
        assert_eq!(Timestamp::from_unix_millis(253_402_300_800_000), None);
    }

    #[test]
    fn refuses_a_time_in_a_different_form() {
        for invalid in [
            "",
            "2026-11-21",
            "2026-11-21T14:02:07Z",
            "2026-11-21T14:02:07.12Z",
            "2026-11-21T14:02:07.1200Z",
            "2026-11-21t14:02:07.120Z",
            "2026-11-21T14:02:07.120z",
            "2026-11-21 14:02:07.120Z",
            "2026-11-21T14:02:07,120Z",
            "2026-11-21T14:02:07.120+00:00",
            "2026/11/21T14:02:07.120Z",
            "2026-11-21T14.02.07.120Z",
            "2026-11-21T14:02:07.120Z ",
            " 2026-11-21T14:02:07.120Z",
            "+026-11-21T14:02:07.120Z",
            "2026-11-21T14:02:07.12\u{662}Z",
            "\u{662}026-11-21T14:02:07.120Z",
            "2026-11-21T14:02:0\u{e9}.20Z",
        ] {
            assert_eq!(
                Timestamp::parse(invalid),
                Err(TimestampError),
                "{invalid:?}"
            );
        }
    }

    #[test]
    fn refuses_a_time_that_does_not_exist() {
        for invalid in [
            "1969-12-31T23:59:59.999Z",
            "2026-00-21T14:02:07.120Z",
            "2026-13-21T14:02:07.120Z",
            "2026-11-00T14:02:07.120Z",
            "2026-11-31T14:02:07.120Z",
            "2026-01-32T14:02:07.120Z",
            "2026-02-29T14:02:07.120Z",
            "2100-02-29T14:02:07.120Z",
            "2024-02-30T14:02:07.120Z",
            "2026-11-21T24:02:07.120Z",
            "2026-11-21T14:60:07.120Z",
            "2026-11-21T14:02:60.120Z",
        ] {
            assert_eq!(
                Timestamp::parse(invalid),
                Err(TimestampError),
                "{invalid:?}"
            );
        }
    }

    #[test]
    fn accepts_the_last_day_of_each_month() {
        for valid in [
            "2026-01-31T00:00:00.000Z",
            "2026-02-28T00:00:00.000Z",
            "2024-02-29T00:00:00.000Z",
            "2026-03-31T00:00:00.000Z",
            "2026-04-30T00:00:00.000Z",
            "2026-05-31T00:00:00.000Z",
            "2026-06-30T00:00:00.000Z",
            "2026-07-31T00:00:00.000Z",
            "2026-08-31T00:00:00.000Z",
            "2026-09-30T00:00:00.000Z",
            "2026-10-31T00:00:00.000Z",
            "2026-11-30T00:00:00.000Z",
            "2026-12-31T23:59:59.999Z",
        ] {
            let time = Timestamp::parse(valid).expect(valid);
            assert_eq!(time.to_string(), valid);
        }
    }

    #[test]
    fn puts_times_in_sequence() {
        let early = Timestamp::parse("2026-11-21T14:02:07.120Z").expect("valid");
        let late = Timestamp::parse("2026-11-21T14:02:07.121Z").expect("valid");
        assert!(early < late);
    }

    #[test]
    fn reads_the_clock_of_the_computer() {
        let before = Timestamp::parse("2026-01-01T00:00:00.000Z").expect("valid");
        assert!(Timestamp::now() > before);
        assert!(Timestamp::now() <= Timestamp::MAX);
    }

    #[test]
    fn describes_the_error() {
        assert_eq!(
            TimestampError.to_string(),
            "a time must have the form 2026-11-21T14:02:07.120Z"
        );
    }
}
