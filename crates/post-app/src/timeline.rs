use jiff::civil::DateTime;
use jiff::tz::TimeZone;
use post_client::Entry;
use post_proto::Timestamp;

use crate::settings::{Clock, Settings};

const UNKNOWN_TIME: &str = "--:--";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClockStyle {
    pub clock: Clock,
    pub seconds: bool,
    pub morning: String,
    pub afternoon: String,
}

impl ClockStyle {
    #[must_use]
    pub fn new(settings: &Settings, morning: String, afternoon: String) -> Self {
        Self {
            clock: settings.clock,
            seconds: settings.seconds,
            morning,
            afternoon,
        }
    }

    fn format(&self, local: &DateTime) -> String {
        let seconds = if self.seconds {
            format!(":{:02}", local.second())
        } else {
            String::new()
        };
        match self.clock {
            Clock::Hours24 => format!("{:02}:{:02}{seconds}", local.hour(), local.minute()),
            Clock::Hours12 => {
                let hour = match local.hour() % 12 {
                    0 => 12,
                    hour => hour,
                };
                let half = if local.hour() < 12 {
                    &self.morning
                } else {
                    &self.afternoon
                };
                format!("{hour:>2}:{:02}{seconds} {half}", local.minute())
            }
        }
    }
}

#[must_use]
pub fn clock(time: Timestamp, zone: &TimeZone, style: &ClockStyle) -> String {
    local(time, zone).map_or_else(|| UNKNOWN_TIME.to_owned(), |local| style.format(&local))
}

#[must_use]
pub fn date_and_clock(time: Timestamp, zone: &TimeZone, style: &ClockStyle) -> String {
    local(time, zone).map_or_else(
        || UNKNOWN_TIME.to_owned(),
        |local| {
            format!(
                "{:04}-{:02}-{:02} {}",
                local.year(),
                local.month(),
                local.day(),
                style.format(&local).trim_start()
            )
        },
    )
}

#[must_use]
pub fn precise_clock(time: Timestamp, zone: &TimeZone) -> String {
    local(time, zone).map_or_else(
        || UNKNOWN_TIME.to_owned(),
        |local| {
            format!(
                "{:02}:{:02}:{:02}.{:03}",
                local.hour(),
                local.minute(),
                local.second(),
                local.millisecond()
            )
        },
    )
}

fn local(time: Timestamp, zone: &TimeZone) -> Option<DateTime> {
    let millis = i64::try_from(time.unix_millis()).ok()?;
    let instant = jiff::Timestamp::from_millisecond(millis).ok()?;
    Some(zone.to_datetime(instant))
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

    use super::{ClockStyle, clock, date_and_clock, name_column, precise_clock, shows_name};
    use crate::settings::{Clock, Settings};

    fn time(text: &str) -> Timestamp {
        Timestamp::parse(text).expect(text)
    }

    fn name(text: &str) -> Name {
        Name::parse(text).expect(text)
    }

    fn style(clock: Clock, seconds: bool) -> ClockStyle {
        ClockStyle {
            clock,
            seconds,
            morning: "AM".to_owned(),
            afternoon: "PM".to_owned(),
        }
    }

    fn plain() -> ClockStyle {
        style(Clock::Hours24, false)
    }

    fn message(from: &str) -> Entry {
        Entry::Message {
            time: time("2026-11-21T14:02:07.120Z"),
            from: name(from),
            text: "Hello".to_owned(),
        }
    }

    #[test]
    fn takes_the_clock_and_the_seconds_from_the_settings() {
        let settings = Settings {
            clock: Clock::Hours12,
            seconds: true,
            ..Settings::default()
        };

        let style = ClockStyle::new(&settings, "a. m.".to_owned(), "p. m.".to_owned());

        assert_eq!(
            style,
            ClockStyle {
                clock: Clock::Hours12,
                seconds: true,
                morning: "a. m.".to_owned(),
                afternoon: "p. m.".to_owned(),
            }
        );
        let default = ClockStyle::new(&Settings::default(), String::new(), String::new());
        assert_eq!(default.clock, Clock::Hours24);
        assert!(!default.seconds);
    }

    #[test]
    fn shows_the_local_time_in_the_24_hour_format() {
        let afternoon = time("2026-11-21T14:02:07.120Z");
        let india = TimeZone::fixed(Offset::from_seconds(19_800).expect("a valid offset"));

        assert_eq!(clock(afternoon, &TimeZone::UTC, &plain()), "14:02");
        assert_eq!(
            clock(afternoon, &TimeZone::fixed(Offset::constant(-6)), &plain()),
            "08:02"
        );
        assert_eq!(clock(afternoon, &india, &plain()), "19:32");
    }

    #[test]
    fn shows_two_digits_for_the_hour_and_the_minute() {
        assert_eq!(
            clock(time("2026-11-21T00:05:59.999Z"), &TimeZone::UTC, &plain()),
            "00:05"
        );
        assert_eq!(
            clock(
                time("2026-11-21T23:30:00.000Z"),
                &TimeZone::fixed(Offset::constant(1)),
                &plain()
            ),
            "00:30"
        );
        assert_eq!(
            clock(time("1970-01-01T00:00:00.000Z"), &TimeZone::UTC, &plain()),
            "00:00"
        );
    }

    #[test]
    fn shows_the_seconds_when_the_style_has_them() {
        let with_seconds = style(Clock::Hours24, true);

        assert_eq!(
            clock(
                time("2026-11-21T14:02:07.120Z"),
                &TimeZone::UTC,
                &with_seconds
            ),
            "14:02:07"
        );
        assert_eq!(
            clock(
                time("2026-11-21T00:05:59.999Z"),
                &TimeZone::UTC,
                &with_seconds
            ),
            "00:05:59"
        );
    }

    #[test]
    fn shows_the_12_hour_format_with_a_constant_width() {
        let twelve = style(Clock::Hours12, false);
        let times = [
            ("2026-11-21T00:05:00.000Z", "12:05 AM"),
            ("2026-11-21T01:00:00.000Z", " 1:00 AM"),
            ("2026-11-21T09:30:00.000Z", " 9:30 AM"),
            ("2026-11-21T11:59:00.000Z", "11:59 AM"),
            ("2026-11-21T12:00:00.000Z", "12:00 PM"),
            ("2026-11-21T14:02:07.120Z", " 2:02 PM"),
            ("2026-11-21T23:59:59.999Z", "11:59 PM"),
        ];
        for (instant, expected) in times {
            assert_eq!(clock(time(instant), &TimeZone::UTC, &twelve), expected);
        }

        let with_seconds = style(Clock::Hours12, true);
        assert_eq!(
            clock(
                time("2026-11-21T14:02:07.120Z"),
                &TimeZone::UTC,
                &with_seconds
            ),
            " 2:02:07 PM"
        );
        let spanish = ClockStyle {
            morning: "a. m.".to_owned(),
            afternoon: "p. m.".to_owned(),
            ..twelve
        };
        assert_eq!(
            clock(time("2026-11-21T09:30:00.000Z"), &TimeZone::UTC, &spanish),
            " 9:30 a. m."
        );
    }

    #[test]
    fn shows_dashes_for_a_time_that_the_calendar_cannot_hold() {
        assert_eq!(clock(Timestamp::MAX, &TimeZone::UTC, &plain()), "--:--");
        assert_eq!(
            date_and_clock(Timestamp::MAX, &TimeZone::UTC, &plain()),
            "--:--"
        );
    }

    #[test]
    fn shows_a_time_with_seconds_and_milliseconds_for_the_console() {
        let instant = time("2026-11-21T14:02:07.120Z");

        assert_eq!(precise_clock(instant, &TimeZone::UTC), "14:02:07.120");
        assert_eq!(
            precise_clock(instant, &TimeZone::fixed(Offset::constant(-6))),
            "08:02:07.120"
        );
        assert_eq!(
            precise_clock(time("2026-11-21T00:00:00.005Z"), &TimeZone::UTC),
            "00:00:00.005"
        );
        assert_eq!(precise_clock(Timestamp::MAX, &TimeZone::UTC), "--:--");
    }

    #[test]
    fn shows_the_local_date_before_the_time() {
        let night = time("2026-11-28T23:30:00.000Z");

        assert_eq!(
            date_and_clock(night, &TimeZone::UTC, &plain()),
            "2026-11-28 23:30"
        );
        assert_eq!(
            date_and_clock(night, &TimeZone::fixed(Offset::constant(1)), &plain()),
            "2026-11-29 00:30"
        );
        assert_eq!(
            date_and_clock(time("2027-01-05T08:07:00.000Z"), &TimeZone::UTC, &plain()),
            "2027-01-05 08:07"
        );
        assert_eq!(
            date_and_clock(
                time("2027-01-05T08:07:09.000Z"),
                &TimeZone::UTC,
                &style(Clock::Hours12, true)
            ),
            "2027-01-05 8:07:09 AM"
        );
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
