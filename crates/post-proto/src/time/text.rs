#[cfg(verus_keep_ghost)]
use vstd::arithmetic::div_mod::{
    lemma_div_denominator, lemma_fundamental_div_mod, lemma_fundamental_div_mod_converse,
    lemma_mod_mod,
};
use vstd::prelude::*;

use super::date::{civil_from_days, days_from_civil, days_in_month};
#[cfg(verus_keep_ghost)]
use super::date::{
    date_of, day_number, last_day, lemma_the_date_of_a_day_gives_the_day,
    lemma_the_day_of_a_date_gives_the_date, valid_date,
};
use crate::ascii::is_digit;

verus! {

pub(super) const LAST_MILLIS: u64 = 253_402_300_799_999;
const MILLIS_IN_SECOND: u64 = 1000;
const MILLIS_IN_MINUTE: u64 = 60_000;
const MILLIS_IN_HOUR: u64 = 3_600_000;
const MILLIS_IN_DAY: u64 = 86_400_000;

pub open spec fn last_millis() -> int {
    253_402_300_799_999
}

pub open spec fn digit(value: int) -> u8 {
    (48 + value) as u8
}

pub open spec fn field_text(
    year: int,
    month: int,
    day: int,
    hour: int,
    minute: int,
    second: int,
    milli: int,
) -> Seq<u8> {
    seq![
        digit(year / 1000),
        digit(year / 100 % 10),
        digit(year / 10 % 10),
        digit(year % 10),
        b'-',
        digit(month / 10),
        digit(month % 10),
        b'-',
        digit(day / 10),
        digit(day % 10),
        b'T',
        digit(hour / 10),
        digit(hour % 10),
        b':',
        digit(minute / 10),
        digit(minute % 10),
        b':',
        digit(second / 10),
        digit(second % 10),
        b'.',
        digit(milli / 100),
        digit(milli / 10 % 10),
        digit(milli % 10),
        b'Z',
    ]
}

pub open spec fn time_text(millis: int) -> Seq<u8> {
    let date = date_of(millis / 86_400_000);
    let in_day = millis % 86_400_000;
    field_text(
        date.0,
        date.1,
        date.2,
        in_day / 3_600_000,
        in_day % 3_600_000 / 60_000,
        in_day % 60_000 / 1000,
        in_day % 1000,
    )
}


pub open spec fn pair(text: Seq<u8>, at: int) -> int {
    (text[at] - 48) * 10 + (text[at + 1] - 48)
}

pub open spec fn has_the_form(text: Seq<u8>) -> bool {
    &&& text.len() == 24
    &&& text[4] == b'-' && text[7] == b'-' && text[10] == b'T'
    &&& text[13] == b':' && text[16] == b':' && text[19] == b'.' && text[23] == b'Z'
    &&& is_digit(text[0]) && is_digit(text[1]) && is_digit(text[2]) && is_digit(text[3])
    &&& is_digit(text[5]) && is_digit(text[6]) && is_digit(text[8]) && is_digit(text[9])
    &&& is_digit(text[11]) && is_digit(text[12]) && is_digit(text[14]) && is_digit(text[15])
    &&& is_digit(text[17]) && is_digit(text[18])
    &&& is_digit(text[20]) && is_digit(text[21]) && is_digit(text[22])
}

pub open spec fn text_year(text: Seq<u8>) -> int {
    pair(text, 0) * 100 + pair(text, 2)
}

pub open spec fn text_milli(text: Seq<u8>) -> int {
    (text[20] - 48) * 100 + pair(text, 21)
}

pub open spec fn has_valid_fields(text: Seq<u8>) -> bool {
    &&& valid_date(text_year(text), pair(text, 5), pair(text, 8))
    &&& pair(text, 11) <= 23 && pair(text, 14) <= 59 && pair(text, 17) <= 59
}

pub open spec fn text_millis(text: Seq<u8>) -> int {
    day_number(text_year(text), pair(text, 5), pair(text, 8)) * 86_400_000
        + pair(text, 11) * 3_600_000 + pair(text, 14) * 60_000 + pair(text, 17) * 1000
        + text_milli(text)
}

proof fn lemma_the_digits_of_a_pair(text: Seq<u8>, at: int)
    requires
        0 <= at,
        at + 1 < text.len(),
        is_digit(text[at]),
        is_digit(text[at + 1]),
    ensures
        0 <= pair(text, at) <= 99,
        digit(pair(text, at) / 10) == text[at],
        digit(pair(text, at) % 10) == text[at + 1],
{
}

proof fn lemma_the_digits_of_two_pairs(high: int, low: int)
    requires
        0 <= high <= 99,
        0 <= low <= 99,
    ensures
        (high * 100 + low) / 1000 == high / 10,
        (high * 100 + low) / 100 % 10 == high % 10,
        (high * 100 + low) / 10 % 10 == low / 10,
        (high * 100 + low) % 10 == low % 10,
{
    let number = high * 100 + low;
    lemma_fundamental_div_mod(high, 10);
    lemma_fundamental_div_mod(low, 10);
    lemma_fundamental_div_mod_converse(number, 1000, high / 10, high % 10 * 100 + low);
    lemma_fundamental_div_mod_converse(number, 100, high, low);
    lemma_fundamental_div_mod_converse(number, 10, high * 10 + low / 10, low % 10);
    lemma_fundamental_div_mod_converse(high * 10 + low / 10, 10, high, low / 10);
}

proof fn lemma_the_digits_of_a_digit_and_a_pair(high: int, low: int)
    requires
        0 <= high <= 9,
        0 <= low <= 99,
    ensures
        (high * 100 + low) / 100 == high,
        (high * 100 + low) / 10 % 10 == low / 10,
        (high * 100 + low) % 10 == low % 10,
{
    let number = high * 100 + low;
    lemma_fundamental_div_mod(low, 10);
    lemma_fundamental_div_mod_converse(number, 100, high, low);
    lemma_fundamental_div_mod_converse(number, 10, high * 10 + low / 10, low % 10);
    lemma_fundamental_div_mod_converse(high * 10 + low / 10, 10, high, low / 10);
}

proof fn lemma_a_number_is_the_sum_of_its_digits(number: int)
    requires
        0 <= number <= 9999,
    ensures
        0 <= number / 1000 <= 9,
        number / 100 * 100 + number / 10 % 10 * 10 + number % 10 == number,
        number / 1000 * 10 + number / 100 % 10 == number / 100,
{
    lemma_fundamental_div_mod(number, 10);
    lemma_fundamental_div_mod(number / 10, 10);
    lemma_div_denominator(number, 10, 10);
    lemma_fundamental_div_mod(number / 100, 10);
    lemma_div_denominator(number, 100, 10);
}

proof fn lemma_a_time_is_the_sum_of_its_parts(millis: int)
    requires
        0 <= millis,
    ensures
        ({
            let in_day = millis % 86_400_000;
            let hour = in_day / 3_600_000;
            let minute = in_day % 3_600_000 / 60_000;
            let second = in_day % 60_000 / 1000;
            let milli = in_day % 1000;
            &&& 0 <= hour <= 23
            &&& 0 <= minute <= 59
            &&& 0 <= second <= 59
            &&& 0 <= milli <= 999
            &&& millis / 86_400_000 * 86_400_000 + hour * 3_600_000 + minute * 60_000 + second
                * 1000 + milli == millis
        }),
{
    let in_day = millis % 86_400_000;
    lemma_fundamental_div_mod(millis, 86_400_000);
    lemma_fundamental_div_mod(in_day, 3_600_000);
    lemma_fundamental_div_mod(in_day % 3_600_000, 60_000);
    lemma_fundamental_div_mod(in_day % 60_000, 1000);
    lemma_mod_mod(in_day, 60_000, 60);
    lemma_mod_mod(in_day, 1000, 60);
}

proof fn lemma_the_parts_of_a_time(days: int, hour: int, minute: int, second: int, milli: int)
    requires
        0 <= days,
        0 <= hour <= 23,
        0 <= minute <= 59,
        0 <= second <= 59,
        0 <= milli <= 999,
    ensures
        ({
            let millis = days * 86_400_000 + hour * 3_600_000 + minute * 60_000 + second * 1000
                + milli;
            let in_day = millis % 86_400_000;
            &&& millis / 86_400_000 == days
            &&& in_day / 3_600_000 == hour
            &&& in_day % 3_600_000 / 60_000 == minute
            &&& in_day % 60_000 / 1000 == second
            &&& in_day % 1000 == milli
        }),
{
    let in_minute = second * 1000 + milli;
    let in_hour = minute * 60_000 + in_minute;
    let in_day = hour * 3_600_000 + in_hour;
    let millis = days * 86_400_000 + in_day;
    lemma_fundamental_div_mod_converse(millis, 86_400_000, days, in_day);
    lemma_fundamental_div_mod_converse(in_day, 3_600_000, hour, in_hour);
    lemma_fundamental_div_mod_converse(in_hour, 60_000, minute, in_minute);
    lemma_fundamental_div_mod_converse(in_day, 60_000, hour * 60 + minute, in_minute);
    lemma_fundamental_div_mod_converse(in_minute, 1000, second, milli);
    lemma_fundamental_div_mod_converse(in_day, 1000, hour * 3600 + minute * 60 + second, milli);
}

proof fn lemma_the_fields_of_a_field_text(
    year: int,
    month: int,
    day: int,
    hour: int,
    minute: int,
    second: int,
    milli: int,
)
    requires
        0 <= year <= 9999,
        0 <= month <= 99,
        0 <= day <= 99,
        0 <= hour <= 99,
        0 <= minute <= 99,
        0 <= second <= 99,
        0 <= milli <= 999,
    ensures
        ({
            let text = field_text(year, month, day, hour, minute, second, milli);
            &&& has_the_form(text)
            &&& text_year(text) == year
            &&& pair(text, 5) == month
            &&& pair(text, 8) == day
            &&& pair(text, 11) == hour
            &&& pair(text, 14) == minute
            &&& pair(text, 17) == second
            &&& text_milli(text) == milli
        }),
{
    lemma_a_number_is_the_sum_of_its_digits(year);
    lemma_a_number_is_the_sum_of_its_digits(milli);
    lemma_fundamental_div_mod(month, 10);
    lemma_fundamental_div_mod(day, 10);
    lemma_fundamental_div_mod(hour, 10);
    lemma_fundamental_div_mod(minute, 10);
    lemma_fundamental_div_mod(second, 10);
}

proof fn lemma_a_text_in_the_form_is_the_text_of_its_fields(text: Seq<u8>)
    requires
        has_the_form(text),
    ensures
        0 <= text_year(text) <= 9999,
        0 <= pair(text, 5) <= 99,
        0 <= pair(text, 8) <= 99,
        0 <= pair(text, 11) <= 99,
        0 <= pair(text, 14) <= 99,
        0 <= pair(text, 17) <= 99,
        0 <= text_milli(text) <= 999,
        field_text(
            text_year(text),
            pair(text, 5),
            pair(text, 8),
            pair(text, 11),
            pair(text, 14),
            pair(text, 17),
            text_milli(text),
        ) =~= text,
{
    lemma_the_digits_of_a_pair(text, 0);
    lemma_the_digits_of_a_pair(text, 2);
    lemma_the_digits_of_a_pair(text, 5);
    lemma_the_digits_of_a_pair(text, 8);
    lemma_the_digits_of_a_pair(text, 11);
    lemma_the_digits_of_a_pair(text, 14);
    lemma_the_digits_of_a_pair(text, 17);
    lemma_the_digits_of_a_pair(text, 21);
    lemma_the_digits_of_two_pairs(pair(text, 0), pair(text, 2));
    lemma_the_digits_of_a_digit_and_a_pair(text[20] - 48, pair(text, 21));
}

proof fn lemma_a_time_gives_a_text_with_its_fields(millis: int)
    requires
        0 <= millis <= last_millis(),
    ensures
        has_the_form(time_text(millis)),
        has_valid_fields(time_text(millis)),
        text_millis(time_text(millis)) == millis,
{
    hide(date_of);
    hide(day_number);
    let days = millis / 86_400_000;
    let date = date_of(days);
    let in_day = millis % 86_400_000;
    let hour = in_day / 3_600_000;
    let minute = in_day % 3_600_000 / 60_000;
    let second = in_day % 60_000 / 1000;
    let milli = in_day % 1000;
    let text = field_text(date.0, date.1, date.2, hour, minute, second, milli);
    assert(time_text(millis) == text);
    assert(0 <= days <= last_day());
    lemma_the_date_of_a_day_gives_the_day(days);
    lemma_a_time_is_the_sum_of_its_parts(millis);
    lemma_the_fields_of_a_field_text(date.0, date.1, date.2, hour, minute, second, milli);
    assert(day_number(text_year(text), pair(text, 5), pair(text, 8)) == days);
}

proof fn lemma_a_text_with_valid_fields_is_the_text_of_its_time(text: Seq<u8>)
    requires
        has_the_form(text),
        has_valid_fields(text),
    ensures
        0 <= text_millis(text) <= last_millis(),
        time_text(text_millis(text)) == text,
{
    hide(date_of);
    hide(day_number);
    let days = day_number(text_year(text), pair(text, 5), pair(text, 8));
    lemma_a_text_in_the_form_is_the_text_of_its_fields(text);
    lemma_the_day_of_a_date_gives_the_date(text_year(text), pair(text, 5), pair(text, 8));
    lemma_the_parts_of_a_time(
        days,
        pair(text, 11),
        pair(text, 14),
        pair(text, 17),
        text_milli(text),
    );
}

pub proof fn lemma_the_text_of_each_time(text: Seq<u8>)
    ensures
        has_the_form(text) && has_valid_fields(text) ==> 0 <= text_millis(text) <= last_millis(),
        forall|time: int|
            0 <= time <= last_millis() ==> (text == #[trigger] time_text(time) <==> has_the_form(text)
                && has_valid_fields(text) && text_millis(text) == time),
{
    if has_the_form(text) && has_valid_fields(text) {
        lemma_a_text_with_valid_fields_is_the_text_of_its_time(text);
    }
    assert forall|time: int| 0 <= time <= last_millis() implies (text == #[trigger] time_text(time)
        <==> has_the_form(text) && has_valid_fields(text) && text_millis(text) == time) by {
        lemma_a_time_gives_a_text_with_its_fields(time);
    }
}

#[expect(clippy::cast_possible_truncation, reason = "Verus proves that the value is a digit")]
fn digit_byte(value: u64) -> (byte: u8)
    requires
        value <= 9,
    ensures
        byte == digit(value as int),
{
    b'0' + value as u8
}

pub(super) fn write_text(millis: u64) -> (text: [u8; 24])
    requires
        millis <= last_millis(),
    ensures
        text@ == time_text(millis as int),
{
    let (year, month, day) = civil_from_days(millis / MILLIS_IN_DAY);
    let in_day = millis % MILLIS_IN_DAY;
    let hour = in_day / MILLIS_IN_HOUR;
    let minute = in_day % MILLIS_IN_HOUR / MILLIS_IN_MINUTE;
    let second = in_day % MILLIS_IN_MINUTE / MILLIS_IN_SECOND;
    let milli = in_day % MILLIS_IN_SECOND;
    [
        digit_byte(year / 1000),
        digit_byte(year / 100 % 10),
        digit_byte(year / 10 % 10),
        digit_byte(year % 10),
        b'-',
        digit_byte(month / 10),
        digit_byte(month % 10),
        b'-',
        digit_byte(day / 10),
        digit_byte(day % 10),
        b'T',
        digit_byte(hour / 10),
        digit_byte(hour % 10),
        b':',
        digit_byte(minute / 10),
        digit_byte(minute % 10),
        b':',
        digit_byte(second / 10),
        digit_byte(second % 10),
        b'.',
        digit_byte(milli / 100),
        digit_byte(milli / 10 % 10),
        digit_byte(milli % 10),
        b'Z',
    ]
}

fn digit_value(byte: u8) -> (value: Option<u64>)
    ensures
        value is Some <==> is_digit(byte),
        value matches Some(value) ==> value == byte - 48,
{
    if is_digit(byte) {
        Some(u64::from(byte - b'0'))
    } else {
        None
    }
}

fn pair_value(text: &[u8], at: usize) -> (value: Option<u64>)
    requires
        at + 1 < text.len(),
    ensures
        value is Some <==> is_digit(text@[at as int]) && is_digit(text@[at + 1]),
        value matches Some(value) ==> value == pair(text@, at as int),
{
    let tens = digit_value(text[at])?;
    let ones = digit_value(text[at + 1])?;
    Some(tens * 10 + ones)
}

pub(super) fn read_text(text: &[u8]) -> (millis: Option<u64>)
    ensures
        millis is Some <==> has_the_form(text@) && has_valid_fields(text@),
        millis matches Some(millis) ==> millis == text_millis(text@),
{
    if text.len() != 24 {
        return None;
    }
    if text[4] != b'-' || text[7] != b'-' || text[10] != b'T' || text[13] != b':' || text[16]
        != b':' || text[19] != b'.' || text[23] != b'Z' {
        return None;
    }
    let century = pair_value(text, 0)?;
    let year_in_century = pair_value(text, 2)?;
    let month = pair_value(text, 5)?;
    let day = pair_value(text, 8)?;
    let hour = pair_value(text, 11)?;
    let minute = pair_value(text, 14)?;
    let second = pair_value(text, 17)?;
    let hundreds = digit_value(text[20])?;
    let milli = hundreds * 100 + pair_value(text, 21)?;
    let year = century * 100 + year_in_century;
    let has_date = year >= 1970 && (1..=12).contains(&month)
        && (1..=days_in_month(year, month)).contains(&day);
    let has_time = hour <= 23 && minute <= 59 && second <= 59;
    if !has_date || !has_time {
        return None;
    }
    Some(
        days_from_civil(year, month, day) * MILLIS_IN_DAY + hour * MILLIS_IN_HOUR + minute
            * MILLIS_IN_MINUTE + second * MILLIS_IN_SECOND + milli,
    )
}

} // verus!
