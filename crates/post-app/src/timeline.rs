use jiff::tz::TimeZone;
use post_client::Entry;
use post_proto::Timestamp;

const UNKNOWN_TIME: &str = "--:--";

#[must_use]
pub fn clock(time: Timestamp, zone: &TimeZone) -> String {
    i64::try_from(time.unix_millis())
        .ok()
        .and_then(|millis| jiff::Timestamp::from_millisecond(millis).ok())
        .map_or_else(
            || UNKNOWN_TIME.to_owned(),
            |instant| {
                let local = zone.to_datetime(instant);
                format!("{:02}:{:02}", local.hour(), local.minute())
            },
        )
}

#[must_use]
pub fn shows_name(entries: &[Entry], index: usize) -> bool {
    let Some(Entry::Message { from, .. }) = entries.get(index) else {
        return false;
    };
    let previous = index.checked_sub(1).and_then(|index| entries.get(index));
    !matches!(previous, Some(Entry::Message { from: earlier, .. }) if earlier == from)
}

#[must_use]
pub fn name_column(entries: &[Entry]) -> usize {
    entries
        .iter()
        .filter_map(|entry| match entry {
            Entry::Message { from, .. } => Some(from.as_str().len()),
            Entry::Joined { .. } | Entry::Left { .. } | Entry::Renamed { .. } => None,
        })
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use jiff::tz::{Offset, TimeZone};
    use post_client::Entry;
    use post_proto::{Name, Timestamp};

    use super::{clock, name_column, shows_name};

    fn time(text: &str) -> Timestamp {
        Timestamp::parse(text).expect(text)
    }

    fn name(text: &str) -> Name {
        Name::parse(text).expect(text)
    }

    fn message(from: &str) -> Entry {
        Entry::Message {
            time: time("2026-11-21T14:02:07.120Z"),
            from: name(from),
            text: "Hello".to_owned(),
        }
    }

    #[test]
    fn shows_the_local_time_in_the_24_hour_format() {
        let afternoon = time("2026-11-21T14:02:07.120Z");
        let india = TimeZone::fixed(Offset::from_seconds(19_800).expect("a valid offset"));

        assert_eq!(clock(afternoon, &TimeZone::UTC), "14:02");
        assert_eq!(
            clock(afternoon, &TimeZone::fixed(Offset::constant(-6))),
            "08:02"
        );
        assert_eq!(clock(afternoon, &india), "19:32");
    }

    #[test]
    fn shows_two_digits_for_the_hour_and_the_minute() {
        assert_eq!(
            clock(time("2026-11-21T00:05:59.999Z"), &TimeZone::UTC),
            "00:05"
        );
        assert_eq!(
            clock(
                time("2026-11-21T23:30:00.000Z"),
                &TimeZone::fixed(Offset::constant(1))
            ),
            "00:30"
        );
        assert_eq!(
            clock(time("1970-01-01T00:00:00.000Z"), &TimeZone::UTC),
            "00:00"
        );
    }

    #[test]
    fn shows_dashes_for_a_time_that_the_calendar_cannot_hold() {
        assert_eq!(clock(Timestamp::MAX, &TimeZone::UTC), "--:--");
    }

    #[test]
    fn shows_the_name_only_on_the_first_message_of_a_group() {
        let entries = [
            message("ana"),
            message("ana"),
            message("ben"),
            Entry::Joined {
                time: time("2026-11-21T14:03:00.000Z"),
                name: name("cy"),
            },
            message("ben"),
            Entry::Left {
                time: time("2026-11-21T14:04:00.000Z"),
                name: name("cy"),
            },
            message("ana"),
            message("ana"),
            Entry::Renamed {
                time: time("2026-11-21T14:05:00.000Z"),
                old: name("ana"),
                new: name("anna"),
            },
            message("anna"),
        ];

        let shown: Vec<bool> = (0..entries.len())
            .map(|index| shows_name(&entries, index))
            .collect();

        assert_eq!(
            shown,
            [
                true, false, true, false, true, false, true, false, false, true
            ]
        );
    }

    #[test]
    fn measures_the_longest_name_of_a_sender() {
        assert_eq!(name_column(&[]), 0);
        assert_eq!(
            name_column(&[Entry::Joined {
                time: time("2026-11-21T14:03:00.000Z"),
                name: name("a-very-long-name"),
            }]),
            0
        );
        assert_eq!(
            name_column(&[message("ana"), message("benjamin"), message("cy")]),
            8
        );
        assert_eq!(
            name_column(&[Entry::Renamed {
                time: time("2026-11-21T14:03:00.000Z"),
                old: name("ana"),
                new: name("a-very-long-name"),
            }]),
            0
        );
    }

    #[test]
    fn shows_no_name_for_an_index_that_does_not_exist() {
        assert!(!shows_name(&[], 0));
        assert!(!shows_name(&[message("ana")], 1));
    }
}
