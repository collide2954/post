use vstd::prelude::*;

verus! {

const DAYS_FROM_YEAR_ZERO_TO_1970: u64 = 719_468;
const DAYS_IN_400_YEARS: u64 = 146_097;

pub open spec fn last_day() -> int {
    2_932_896
}

pub open spec fn leap_year(year: int) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

pub open spec fn month_length(year: int, month: int) -> int {
    if month == 4 || month == 6 || month == 9 || month == 11 {
        30
    } else if month == 2 {
        if leap_year(year) { 29 } else { 28 }
    } else {
        31
    }
}

pub open spec fn valid_date(year: int, month: int, day: int) -> bool {
    1970 <= year <= 9999 && 1 <= month <= 12 && 1 <= day <= month_length(year, month)
}

pub open spec fn day_number(year: int, month: int, day: int) -> int {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year / 400;
    let year_of_era = year % 400;
    let month_from_march = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_from_march + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

pub open spec fn era_year(day_of_era: int) -> int {
    (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365
}

pub open spec fn year_day(day_of_era: int) -> int {
    let year_of_era = era_year(day_of_era);
    day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100)
}

pub open spec fn date_of(days: int) -> (int, int, int) {
    let days = days + 719_468;
    let era = days / 146_097;
    let day_of_era = days % 146_097;
    let year_of_era = era_year(day_of_era);
    let day_of_year = year_day(day_of_era);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 { month_from_march + 3 } else { month_from_march - 9 };
    let year = year_of_era + era * 400 + if month <= 2 { 1int } else { 0int };
    (year, month, day)
}

pub open spec fn era_day(year_of_era: int, day_of_year: int) -> int {
    year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year
}

pub open spec fn has_leap_day(year_of_era: int) -> bool {
    year_of_era % 4 == 3 && (year_of_era % 100 != 99 || year_of_era == 399)
}

proof fn lemma_the_year_of_an_era_day(century: int, group: int, rest: int, day_of_year: int)
    requires
        0 <= century <= 3,
        0 <= group <= 24,
        0 <= rest <= 3,
        0 <= day_of_year <= 365,
        day_of_year <= 364 || has_leap_day(100 * century + 4 * group + rest),
    ensures
        era_year(era_day(100 * century + 4 * group + rest, day_of_year))
            == 100 * century + 4 * group + rest,
{
    let year_of_era = 100 * century + 4 * group + rest;
    let day_of_era = era_day(year_of_era, day_of_year);
    assert(year_of_era / 4 == 25 * century + group);
    assert(year_of_era / 100 == century);
    assert(day_of_era == 36_524 * century + 1461 * group + 365 * rest + day_of_year);
    assert(day_of_era - 365 * year_of_era == 24 * century + group + day_of_year);
}

proof fn lemma_the_parts_of_an_era_day(day_of_era: int)
    requires
        0 <= day_of_era <= 146_096,
    ensures
        0 <= era_year(day_of_era) <= 399,
        0 <= year_day(day_of_era) <= 365,
        year_day(day_of_era) <= 364 || has_leap_day(era_year(day_of_era)),
{
}

pub proof fn lemma_the_date_of_a_day_gives_the_day(days: int)
    requires
        0 <= days <= last_day(),
    ensures
        ({
            let date = date_of(days);
            valid_date(date.0, date.1, date.2) && day_number(date.0, date.1, date.2) == days
        }),
{
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let day_of_era = shifted % 146_097;
    let year_of_era = era_year(day_of_era);
    let day_of_year = year_day(day_of_era);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let month = if month_from_march < 10 { month_from_march + 3 } else { month_from_march - 9 };
    let year = year_of_era + era * 400 + if month <= 2 { 1int } else { 0int };
    lemma_the_parts_of_an_era_day(day_of_era);
    assert(0 <= month_from_march <= 11);
    let march_year = if month <= 2 { year - 1 } else { year };
    assert(march_year == year_of_era + era * 400);
    assert(march_year / 400 == era && march_year % 400 == year_of_era);
}

pub proof fn lemma_the_day_of_a_date_gives_the_date(year: int, month: int, day: int)
    requires
        valid_date(year, month, day),
    ensures
        0 <= day_number(year, month, day) <= last_day(),
        date_of(day_number(year, month, day)) == (year, month, day),
{
    let march_year = if month <= 2 { year - 1 } else { year };
    let era = march_year / 400;
    let year_of_era = march_year % 400;
    let month_from_march = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_from_march + 2) / 5 + day - 1;
    let day_of_era = era_day(year_of_era, day_of_year);
    let century = year_of_era / 100;
    let group = (year_of_era % 100) / 4;
    let rest = year_of_era % 4;
    assert(year_of_era == 100 * century + 4 * group + rest);
    assert(0 <= day_of_year <= 365);
    assert(day_of_year == 365 ==> month == 2 && day == 29);
    assert(day_of_year <= 364 || has_leap_day(year_of_era));
    lemma_the_year_of_an_era_day(century, group, rest, day_of_year);
    assert(era_year(day_of_era) == year_of_era);
    assert(0 <= day_of_era <= 146_096);
    let shifted = era * 146_097 + day_of_era;
    assert(shifted / 146_097 == era && shifted % 146_097 == day_of_era);
    assert((5 * day_of_year + 2) / 153 == month_from_march);
}

fn is_leap_year(year: u64) -> (leap: bool)
    ensures
        leap == leap_year(year as int),
{
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

pub(super) fn days_in_month(year: u64, month: u64) -> (days: u64)
    ensures
        days == month_length(year as int, month as int),
{
    match month {
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 31,
    }
}

pub(super) fn days_from_civil(year: u64, month: u64, day: u64) -> (days: u64)
    requires
        valid_date(year as int, month as int, day as int),
    ensures
        days == day_number(year as int, month as int, day as int),
        days <= last_day(),
{
    let year = if month <= 2 { year - 1 } else { year };
    let era = year / 400;
    let year_of_era = year % 400;
    let month_from_march = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_from_march + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * DAYS_IN_400_YEARS + day_of_era - DAYS_FROM_YEAR_ZERO_TO_1970
}

pub(super) fn civil_from_days(days: u64) -> (date: (u64, u64, u64))
    requires
        days <= last_day(),
    ensures
        (date.0 as int, date.1 as int, date.2 as int) == date_of(days as int),
        valid_date(date.0 as int, date.1 as int, date.2 as int),
{
    proof {
        lemma_the_date_of_a_day_gives_the_day(days as int);
    }
    let days = days + DAYS_FROM_YEAR_ZERO_TO_1970;
    let era = days / DAYS_IN_400_YEARS;
    let day_of_era = days % DAYS_IN_400_YEARS;
    proof {
        lemma_the_parts_of_an_era_day(day_of_era as int);
    }
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 { month_from_march + 3 } else { month_from_march - 9 };
    let year = year_of_era + era * 400;
    if month <= 2 {
        (year + 1, month, day)
    } else {
        (year, month, day)
    }
}

} // verus!
