use chrono::{prelude::*, Duration, LocalResult};
use std::fmt::Display;

use crate::{define_jsclass_with_symbol, prelude::*, vm::context::Context};

/// The number of nanoseconds in a millisecond.
const NANOS_PER_MS: i64 = 1_000_000;
/// The number of milliseconds in an hour.
const MILLIS_PER_HOUR: i64 = 3_600_000;
/// The number of milliseconds in a minute.
const MILLIS_PER_MINUTE: i64 = 60_000;
/// The number of milliseconds in a second.
const MILLIS_PER_SECOND: i64 = 1000;

#[inline]
fn is_zero_or_normal_opt(value: Option<f64>) -> bool {
    value
        .map(|value| value == 0f64 || value.is_normal())
        .unwrap_or(true)
}

macro_rules! check_normal_opt {
    ($($v:expr),+) => {
        $(is_zero_or_normal_opt($v.into()) &&)+ true
    };
}

#[inline]
fn ignore_ambiguity<T>(result: LocalResult<T>) -> Option<T> {
    match result {
        LocalResult::Ambiguous(v, _) => Some(v),
        LocalResult::Single(v) => Some(v),
        LocalResult::None => None,
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date(Option<NaiveDateTime>);

impl Display for Date {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.to_local() {
            Some(v) => write!(f, "{}", v.format("%a %b %d %Y %H:%M:%S GMT%:z")),
            _ => write!(f, "Invalid Date"),
        }
    }
}
impl Default for Date {
    fn default() -> Self {
        Self(Some(Utc::now().naive_utc()))
    }
}

extern "C" fn fsz() -> usize {
    std::mem::size_of::<Date>()
}

extern "C" fn ser(_: &JsObject, _: &mut SnapshotSerializer) {
    todo!()
}

extern "C" fn deser(_: &mut JsObject, _: &mut Deserializer, _: &mut Context) {
    todo!()
}

impl Date {
    define_jsclass_with_symbol!(
        JsObject,
        Date,
        Date,
        None,
        None,
        Some(deser),
        Some(ser),
        Some(fsz)
    );

    /// Check if the time (number of miliseconds) is in the expected range.
    /// Returns None if the time is not in the range, otherwise returns the time itself in option.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-timeclip
    #[inline]
    pub fn time_clip(time: f64) -> Option<f64> {
        if time.abs() > 8.64e15 {
            None
        } else {
            Some(time)
        }
    }

    /// Converts the `Date` to a local `DateTime`.
    ///
    /// If the `Date` is invalid (i.e. NAN), this function will return `None`.
    pub fn to_local(self) -> Option<DateTime<Local>> {
        self.0
            .map(|utc| Local::now().timezone().from_utc_datetime(&utc))
    }

    /// Converts the `Date` to a UTC `DateTime`.
    ///
    /// If the `Date` is invalid (i.e. NAN), this function will return `None`.
    pub fn to_utc(self) -> Option<DateTime<Utc>> {
        self.0
            .map(|utc| Utc::now().timezone().from_utc_datetime(&utc))
    }

    /// Optionally sets the individual components of the `Date`.
    ///
    /// Each component does not have to be within the range of valid values. For example, if `month` is too large
    /// then `year` will be incremented by the required amount.
    #[allow(clippy::too_many_arguments)]
    pub fn set_components(
        &mut self,
        utc: bool,
        year: Option<f64>,
        month: Option<f64>,
        day: Option<f64>,
        hour: Option<f64>,
        minute: Option<f64>,
        second: Option<f64>,
        millisecond: Option<f64>,
    ) {
        #[inline]
        fn num_days_in(year: i32, month: u32) -> Option<u32> {
            let month = month + 1; // zero-based for calculations

            Some(
                NaiveDate::from_ymd_opt(
                    match month {
                        12 => year.checked_add(1)?,
                        _ => year,
                    },
                    match month {
                        12 => 1,
                        _ => month + 1,
                    },
                    1,
                )?
                .signed_duration_since(NaiveDate::from_ymd_opt(year, month, 1)?)
                .num_days() as u32,
            )
        }

        #[inline]
        fn fix_month(year: i32, month: i32) -> Option<(i32, u32)> {
            let year = year.checked_add(month / 12)?;

            if month < 0 {
                let year = year.checked_sub(1)?;
                let month = (11 + (month + 1) % 12) as u32;
                Some((year, month))
            } else {
                let month = (month % 12) as u32;
                Some((year, month))
            }
        }

        #[inline]
        fn fix_day(mut year: i32, mut month: i32, mut day: i32) -> Option<(i32, u32, u32)> {
            loop {
                if day < 0 {
                    let (fixed_year, fixed_month) = fix_month(year, month.checked_sub(1)?)?;

                    year = fixed_year;
                    month = fixed_month as i32;
                    day += num_days_in(fixed_year, fixed_month)? as i32;
                } else {
                    let (fixed_year, fixed_month) = fix_month(year, month)?;
                    let num_days = num_days_in(fixed_year, fixed_month)? as i32;

                    if day >= num_days {
                        day -= num_days;
                        month = month.checked_add(1)?;
                    } else {
                        break;
                    }
                }
            }

            let (fixed_year, fixed_month) = fix_month(year, month)?;
            Some((fixed_year, fixed_month, day as u32))
        }

        // If any of the args are infinity or NaN, return an invalid date.
        if !check_normal_opt!(year, month, day, hour, minute, second, millisecond) {
            self.0 = None;
            return;
        }

        let naive = if utc {
            self.to_utc().map(|dt| dt.naive_utc())
        } else {
            self.to_local().map(|dt| dt.naive_local())
        };

        self.0 = naive.and_then(|naive| {
            let year = year.unwrap_or_else(|| naive.year() as f64) as i32;
            let month = month.unwrap_or_else(|| naive.month0() as f64) as i32;
            let day = (day.unwrap_or_else(|| naive.day() as f64) as i32).checked_sub(1)?;
            let hour = hour.unwrap_or_else(|| naive.hour() as f64) as i64;
            let minute = minute.unwrap_or_else(|| naive.minute() as f64) as i64;
            let second = second.unwrap_or_else(|| naive.second() as f64) as i64;
            let millisecond = millisecond
                .unwrap_or_else(|| naive.nanosecond() as f64 / NANOS_PER_MS as f64)
                as i64;

            let (year, month, day) = fix_day(year, month, day)?;

            let duration_hour = Duration::milliseconds(hour.checked_mul(MILLIS_PER_HOUR)?);
            let duration_minute = Duration::milliseconds(minute.checked_mul(MILLIS_PER_MINUTE)?);
            let duration_second = Duration::milliseconds(second.checked_mul(MILLIS_PER_SECOND)?);
            let duration_milisecond = Duration::milliseconds(millisecond);

            let duration = duration_hour
                .checked_add(&duration_minute)?
                .checked_add(&duration_second)?
                .checked_add(&duration_milisecond)?;

            NaiveDate::from_ymd_opt(year, month + 1, day + 1)
                .and_then(|dt| dt.and_hms(0, 0, 0).checked_add_signed(duration))
                .and_then(|dt| {
                    if utc {
                        Some(Utc.from_utc_datetime(&dt).naive_utc())
                    } else {
                        ignore_ambiguity(Local.from_local_datetime(&dt)).map(|dt| dt.naive_utc())
                    }
                })
                .filter(|dt| Self::time_clip(dt.timestamp_millis() as f64).is_some())
        });
    }
}
