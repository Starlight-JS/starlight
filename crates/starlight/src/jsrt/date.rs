use chrono::{prelude::*, Duration, LocalResult};
use std::{
    fmt::Display,
    intrinsics::transmute,
    mem::{size_of, ManuallyDrop},
};

use crate::{
    define_jsclass_with_symbol,
    prelude::*,
    vm::{class::JsClass, context::Context, object::TypedJsObject},
    JsTryFrom,
};

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

extern "C" fn ser(object: &JsObject, ser: &mut SnapshotSerializer) {
    let date = **object.data::<Date>();
    match date.0 {
        Some(time) => {
            ser.write_u8(0x1);
            let bytes: [u8; size_of::<NaiveDateTime>()] = unsafe { transmute(time) };
            for byte in bytes {
                ser.write_u8(byte);
            }
        }
        None => {
            ser.write_u8(0x0);
        }
    }
}

extern "C" fn deser(object: &mut JsObject, deser: &mut Deserializer) {
    let is_valid = deser.get_u8();
    match is_valid {
        0x0 => *object.data::<Date>() = ManuallyDrop::new(Date(None)),
        0x1 => unsafe {
            let mut bytes: [u8; size_of::<NaiveDateTime>()] = [0; size_of::<NaiveDateTime>()];
            for i in 0..bytes.len() {
                bytes[i] = deser.get_u8();
            }
            *object.data::<Date>() = ManuallyDrop::new(Date(Some(transmute(bytes))));
        },
        _ => unreachable!(),
    }
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
    /// `Date.prototype.toDateString()`
    ///
    /// The `toDateString()` method returns the date portion of a Date object in English.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.todatestring
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/toDateString
    pub fn to_date_string(self) -> String {
        self.to_local()
            .map(|date_time| date_time.format("%a %b %d %Y").to_string())
            .unwrap_or_else(|| "Invalid Date".to_string())
    }

    /// `Date.prototype.toGMTString()`
    ///
    /// The `toGMTString()` method converts a date to a string, using Internet Greenwich Mean Time (GMT) conventions.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.togmtstring
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/toGMTString
    pub fn to_gmt_string(self) -> String {
        self.to_utc_string()
    }

    /// `Date.prototype.toISOString()`
    ///
    /// The `toISOString()` method returns a string in simplified extended ISO format (ISO 8601).
    ///
    /// More information:
    ///  - [ISO 8601][iso8601]
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [iso8601]: http://en.wikipedia.org/wiki/ISO_8601
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.toisostring
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/toISOString
    pub fn to_iso_string(self) -> String {
        self.to_utc()
            // RFC 3389 uses +0.00 for UTC, where JS expects Z, so we can't use the built-in chrono function.
            .map(|f| f.format("%Y-%m-%dT%H:%M:%S.%3fZ").to_string())
            .unwrap_or_else(|| "Invalid Date".to_string())
    }

    /// `Date.prototype.toJSON()`
    ///
    /// The `toJSON()` method returns a string representation of the `Date` object.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.tojson
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/toJSON
    pub fn to_json(self) -> String {
        self.to_iso_string()
    }

    /// `Date.prototype.toTimeString()`
    ///
    /// The `toTimeString()` method returns the time portion of a Date object in human readable form in American
    /// English.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.totimestring
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/toTimeString
    pub fn to_time_string(self) -> String {
        self.to_local()
            .map(|date_time| date_time.format("%H:%M:%S GMT%:z").to_string())
            .unwrap_or_else(|| "Invalid Date".to_string())
    }

    /// `Date.prototype.toUTCString()`
    ///
    /// The `toUTCString()` method returns a string representing the specified Date object.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.toutcstring
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/toUTCString
    pub fn to_utc_string(self) -> String {
        self.to_utc()
            .map(|date_time| date_time.format("%a, %d %b %Y %H:%M:%S GMT").to_string())
            .unwrap_or_else(|| "Invalid Date".to_string())
    }

    /// `Date.prototype.valueOf()`
    ///
    /// The `valueOf()` method returns the primitive value of a `Date` object.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.valueof
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/valueOf
    pub fn value_of(&self) -> f64 {
        self.get_time()
    }

    /// `Date.prototype.getDate()`
    ///
    /// The `getDate()` method returns the day of the month for the specified date according to local time.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.getdate
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/getDate
    pub fn get_date(&self) -> f64 {
        self.to_local().map_or(f64::NAN, |dt| dt.day() as f64)
    }

    /// `Date.prototype.getDay()`
    ///
    /// The `getDay()` method returns the day of the week for the specified date according to local time, where 0
    /// represents Sunday.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.getday
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/getDay
    pub fn get_day(&self) -> f64 {
        self.to_local().map_or(f64::NAN, |dt| {
            let weekday = dt.weekday() as u32;
            let weekday = (weekday + 1) % 7; // 0 represents Monday in Chrono
            weekday as f64
        })
    }

    /// `Date.prototype.getFullYear()`
    ///
    /// The `getFullYear()` method returns the year of the specified date according to local time.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.getfullyear
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/getFullYear
    pub fn get_full_year(&self) -> f64 {
        self.to_local().map_or(f64::NAN, |dt| dt.year() as f64)
    }

    /// `Date.prototype.getHours()`
    ///
    /// The `getHours()` method returns the hour for the specified date, according to local time.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.gethours
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/getHours
    pub fn get_hours(&self) -> f64 {
        self.to_local().map_or(f64::NAN, |dt| dt.hour() as f64)
    }

    /// `Date.prototype.getMilliseconds()`
    ///
    /// The `getMilliseconds()` method returns the milliseconds in the specified date according to local time.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.getmilliseconds
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/getMilliseconds
    pub fn get_milliseconds(&self) -> f64 {
        self.to_local()
            .map_or(f64::NAN, |dt| dt.nanosecond() as f64 / NANOS_PER_MS as f64)
    }

    /// `Date.prototype.getMinutes()`
    ///
    /// The `getMinutes()` method returns the minutes in the specified date according to local time.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.getminutes
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/getMinutes
    pub fn get_minutes(&self) -> f64 {
        self.to_local().map_or(f64::NAN, |dt| dt.minute() as f64)
    }

    /// `Date.prototype.getMonth()`
    ///
    /// The `getMonth()` method returns the month in the specified date according to local time, as a zero-based value
    /// (where zero indicates the first month of the year).
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.getmonth
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/getMonth
    pub fn get_month(&self) -> f64 {
        self.to_local().map_or(f64::NAN, |dt| dt.month0() as f64)
    }

    /// `Date.prototype.getSeconds()`
    ///
    /// The `getSeconds()` method returns the seconds in the specified date according to local time.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.getseconds
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/getSeconds
    pub fn get_seconds(&self) -> f64 {
        self.to_local().map_or(f64::NAN, |dt| dt.second() as f64)
    }

    /// `Date.prototype.getYear()`
    ///
    /// The getYear() method returns the year in the specified date according to local time. Because getYear() does not
    /// return full years ("year 2000 problem"), it is no longer used and has been replaced by the getFullYear() method.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.getyear
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/getYear
    pub fn get_year(&self) -> f64 {
        self.to_local()
            .map_or(f64::NAN, |dt| dt.year() as f64 - 1900f64)
    }

    /// `Date.prototype.getTime()`
    ///
    /// The `getTime()` method returns the number of milliseconds since the Unix Epoch.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.gettime
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/getTime
    pub fn get_time(&self) -> f64 {
        self.to_utc()
            .map_or(f64::NAN, |dt| dt.timestamp_millis() as f64)
    }

    /// `Date.prototype.getTimeZoneOffset()`
    ///
    /// The getTimezoneOffset() method returns the time zone difference, in minutes, from current locale (host system
    /// settings) to UTC.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.gettimezoneoffset
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/getTimezoneOffset
    #[inline]
    pub fn get_timezone_offset() -> f64 {
        let offset_seconds = chrono::Local::now().offset().local_minus_utc() as f64;
        offset_seconds / 60f64
    }

    /// `Date.prototype.getUTCDate()`
    ///
    /// The `getUTCDate()` method returns the day (date) of the month in the specified date according to universal time.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.getutcdate
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/getUTCDate
    pub fn get_utc_date(&self) -> f64 {
        self.to_utc().map_or(f64::NAN, |dt| dt.day() as f64)
    }

    /// `Date.prototype.getUTCDay()`
    ///
    /// The `getUTCDay()` method returns the day of the week in the specified date according to universal time, where 0
    /// represents Sunday.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.getutcday
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/getUTCDay
    pub fn get_utc_day(&self) -> f64 {
        self.to_utc().map_or(f64::NAN, |dt| {
            let weekday = dt.weekday() as u32;
            let weekday = (weekday + 1) % 7; // 0 represents Monday in Chrono
            weekday as f64
        })
    }

    /// `Date.prototype.getUTCFullYear()`
    ///
    /// The `getUTCFullYear()` method returns the year in the specified date according to universal time.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.getutcfullyear
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/getUTCFullYear
    pub fn get_utc_full_year(&self) -> f64 {
        self.to_utc().map_or(f64::NAN, |dt| dt.year() as f64)
    }

    /// `Date.prototype.getUTCHours()`
    ///
    /// The `getUTCHours()` method returns the hours in the specified date according to universal time.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.getutchours
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/getUTCHours
    pub fn get_utc_hours(&self) -> f64 {
        self.to_utc().map_or(f64::NAN, |dt| dt.hour() as f64)
    }

    /// `Date.prototype.getUTCMilliseconds()`
    ///
    /// The `getUTCMilliseconds()` method returns the milliseconds portion of the time object's value.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.getutcmilliseconds
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/getUTCMilliseconds
    pub fn get_utc_milliseconds(&self) -> f64 {
        self.to_utc()
            .map_or(f64::NAN, |dt| dt.nanosecond() as f64 / NANOS_PER_MS as f64)
    }

    /// `Date.prototype.getUTCMinutes()`
    ///
    /// The `getUTCMinutes()` method returns the minutes in the specified date according to universal time.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.getutcminutes
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/getUTCMinutes
    pub fn get_utc_minutes(&self) -> f64 {
        self.to_utc().map_or(f64::NAN, |dt| dt.minute() as f64)
    }

    /// `Date.prototype.getUTCMonth()`
    ///
    /// The `getUTCMonth()` returns the month of the specified date according to universal time, as a zero-based value
    /// (where zero indicates the first month of the year).
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.getutcmonth
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/getUTCMonth
    pub fn get_utc_month(&self) -> f64 {
        self.to_utc().map_or(f64::NAN, |dt| dt.month0() as f64)
    }

    /// `Date.prototype.getUTCSeconds()`
    ///
    /// The `getUTCSeconds()` method returns the seconds in the specified date according to universal time.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.getutcseconds
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/getUTCSeconds
    pub fn get_utc_seconds(&self) -> f64 {
        self.to_utc().map_or(f64::NAN, |dt| dt.second() as f64)
    }

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

    fn make_date_string(ctx: GcPointer<Context>) -> JsValue {
        JsValue::new(JsString::new(ctx, Local::now().to_rfc3339()))
    }
    /// `Date()`
    ///
    /// The newly-created `Date` object represents the current date and time as of the time of instantiation.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date-constructor
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/Date
    fn make_date_now(_ctx: GcPointer<Context>, object: GcPointer<JsObject>) -> JsValue {
        *object.data::<Date>() = ManuallyDrop::new(Date::default());
        JsValue::new(object)
    }

    /// `Date(value)`
    ///
    /// The newly-created `Date` object represents the value provided to the constructor.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date-constructor
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/Date
    fn make_date_single(
        ctx: GcPointer<Context>,
        object: GcPointer<JsObject>,
        value: JsValue,
    ) -> Result<JsValue, JsValue> {
        let tv = match this_time_value(value, ctx) {
            Ok(dt) => dt.0,
            _ => {
                let prim = value.to_primitive(ctx, JsHint::None)?;
                if prim.is_jsstring() {
                    match chrono::DateTime::parse_from_rfc3339(&prim.to_string(ctx)?) {
                        Ok(dt) => Some(dt.naive_utc()),
                        _ => None,
                    }
                } else {
                    let tv = prim.to_number(ctx)?;
                    let secs = (tv / 1_000f64) as i64;
                    let nsecs = ((tv % 1_000f64) * 1_000_000f64) as u32;
                    NaiveDateTime::from_timestamp_opt(secs, nsecs)
                }
            }
        };
        let tv = tv.filter(|time| Self::time_clip(time.timestamp_millis() as f64).is_some());
        let date = Date(tv);
        *object.data::<Date>() = ManuallyDrop::new(date);
        Ok(JsValue::new(object))
    }

    fn make_date_multiple(
        ctx: GcPointer<Context>,
        object: GcPointer<JsObject>,
        args: &Arguments,
    ) -> Result<JsValue, JsValue> {
        let year = args.at(0).to_number(ctx)?;
        let month = args.at(1).to_number(ctx)?;
        let day = args
            .try_at(2)
            .map_or(Ok(1f64), |value| value.to_number(ctx))?;
        let hour = args
            .try_at(3)
            .map_or(Ok(0f64), |value| value.to_number(ctx))?;
        let min = args
            .try_at(4)
            .map_or(Ok(0f64), |value| value.to_number(ctx))?;
        let sec = args
            .try_at(5)
            .map_or(Ok(0f64), |value| value.to_number(ctx))?;
        let milli = args
            .try_at(6)
            .map_or(Ok(0f64), |value| value.to_number(ctx))?;
        // If any of the args are infinity or NaN, return an invalid date.
        if !check_normal_opt!(year, month, day, hour, min, sec, milli) {
            let date = Date(None);
            *object.data::<Self>() = ManuallyDrop::new(date);

            return Ok(JsValue::new(object));
        }

        let year = year as i32;
        let month = month as u32;
        let day = day as u32;
        let hour = hour as u32;
        let min = min as u32;
        let sec = sec as u32;
        let milli = milli as u32;

        let year = if (0..=99).contains(&year) {
            1900 + year
        } else {
            year
        };

        let final_date = NaiveDate::from_ymd_opt(year, month + 1, day)
            .and_then(|naive_date| naive_date.and_hms_milli_opt(hour, min, sec, milli))
            .and_then(|local| ignore_ambiguity(Local.from_local_datetime(&local)))
            .map(|local| local.naive_utc())
            .filter(|time| Self::time_clip(time.timestamp_millis() as f64).is_some());

        let date = Date(final_date);
        *object.data::<Self>() = ManuallyDrop::new(date);
        Ok(JsValue::new(object))
    }

    /// `Date.prototype.setDate()`
    ///
    /// The `setDate()` method sets the day of the `Date` object relative to the beginning of the currently set
    /// month.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.setdate
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/setDate
    pub fn set_date(&mut self, day: Option<f64>) {
        if let Some(day) = day {
            self.set_components(false, None, None, Some(day), None, None, None, None)
        } else {
            self.0 = None
        }
    }

    /// `Date.prototype.setFullYear()`
    ///
    /// The `setFullYear()` method sets the full year for a specified date according to local time. Returns new
    /// timestamp.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.setfullyear
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/setFullYear
    pub fn set_full_year(&mut self, year: Option<f64>, month: Option<f64>, day: Option<f64>) {
        if let Some(year) = year {
            self.set_components(false, Some(year), month, day, None, None, None, None)
        } else {
            self.0 = None
        }
    }

    /// `Date.prototype.setHours()`
    ///
    /// The `setHours()` method sets the hours for a specified date according to local time, and returns the number
    /// of milliseconds since January 1, 1970 00:00:00 UTC until the time represented by the updated `Date`
    /// instance.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.sethours
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/setHours
    pub fn set_hours(
        &mut self,
        hour: Option<f64>,
        minute: Option<f64>,
        second: Option<f64>,
        millisecond: Option<f64>,
    ) {
        if let Some(hour) = hour {
            self.set_components(
                false,
                None,
                None,
                None,
                Some(hour),
                minute,
                second,
                millisecond,
            )
        } else {
            self.0 = None
        }
    }

    /// `Date.prototype.setMilliseconds()`
    ///
    /// The `setMilliseconds()` method sets the milliseconds for a specified date according to local time.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.setmilliseconds
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/setMilliseconds
    pub fn set_milliseconds(&mut self, millisecond: Option<f64>) {
        if let Some(millisecond) = millisecond {
            self.set_components(false, None, None, None, None, None, None, Some(millisecond))
        } else {
            self.0 = None
        }
    }

    /// `Date.prototype.setMinutes()`
    ///
    /// The `setMinutes()` method sets the minutes for a specified date according to local time.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.setminutes
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/setMinutes
    pub fn set_minutes(
        &mut self,
        minute: Option<f64>,
        second: Option<f64>,
        millisecond: Option<f64>,
    ) {
        if let Some(minute) = minute {
            self.set_components(
                false,
                None,
                None,
                None,
                None,
                Some(minute),
                second,
                millisecond,
            )
        } else {
            self.0 = None
        }
    }

    /// `Date.prototype.setMonth()`
    ///
    /// The `setMonth()` method sets the month for a specified date according to the currently set year.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.setmonth
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/setMonth
    pub fn set_month(&mut self, month: Option<f64>, day: Option<f64>) {
        if let Some(month) = month {
            self.set_components(false, None, Some(month), day, None, None, None, None)
        } else {
            self.0 = None
        }
    }

    /// `Date.prototype.setSeconds()`
    ///
    /// The `setSeconds()` method sets the seconds for a specified date according to local time.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.setseconds
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/setSeconds
    pub fn set_seconds(&mut self, second: Option<f64>, millisecond: Option<f64>) {
        if let Some(second) = second {
            self.set_components(
                false,
                None,
                None,
                None,
                None,
                None,
                Some(second),
                millisecond,
            )
        } else {
            self.0 = None
        }
    }

    /// `Date.prototype.setYear()`
    ///
    /// The `setYear()` method sets the year for a specified date according to local time.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.setyear
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/setYear
    pub fn set_year(&mut self, year: Option<f64>, month: Option<f64>, day: Option<f64>) {
        if let Some(mut year) = year {
            year += if (0f64..100f64).contains(&year) {
                1900f64
            } else {
                0f64
            };
            self.set_components(false, Some(year), month, day, None, None, None, None)
        } else {
            self.0 = None
        }
    }

    /// `Date.prototype.setTime()`
    ///
    /// The `setTime()` method sets the Date object to the time represented by a number of milliseconds since
    /// January 1, 1970, 00:00:00 UTC.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.settime
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/setTime
    pub fn set_time(&mut self, time: Option<f64>) {
        if let Some(time) = time {
            let secs = (time / 1_000f64) as i64;
            let nsecs = ((time % 1_000f64) * 1_000_000f64) as u32;
            self.0 = ignore_ambiguity(Local.timestamp_opt(secs, nsecs)).map(|dt| dt.naive_utc());
        } else {
            self.0 = None
        }
    }

    /// `Date.prototype.setUTCDate()`
    ///
    /// The `setUTCDate()` method sets the day of the month for a specified date according to universal time.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.setutcdate
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/setUTCDate
    pub fn set_utc_date(&mut self, day: Option<f64>) {
        if let Some(day) = day {
            self.set_components(true, None, None, Some(day), None, None, None, None)
        } else {
            self.0 = None
        }
    }

    /// `Date.prototype.setFullYear()`
    ///
    /// The `setFullYear()` method sets the full year for a specified date according to local time. Returns new
    /// timestamp.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.setutcfullyear
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/setUTCFullYear
    pub fn set_utc_full_year(&mut self, year: Option<f64>, month: Option<f64>, day: Option<f64>) {
        if let Some(year) = year {
            self.set_components(true, Some(year), month, day, None, None, None, None)
        } else {
            self.0 = None
        }
    }

    /// `Date.prototype.setUTCHours()`
    ///
    /// The `setUTCHours()` method sets the hour for a specified date according to universal time, and returns the
    /// number of milliseconds since  January 1, 1970 00:00:00 UTC until the time represented by the updated `Date`
    /// instance.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.setutchours
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/setUTCHours
    pub fn set_utc_hours(
        &mut self,
        hour: Option<f64>,
        minute: Option<f64>,
        second: Option<f64>,
        millisecond: Option<f64>,
    ) {
        if let Some(hour) = hour {
            self.set_components(
                true,
                None,
                None,
                None,
                Some(hour),
                minute,
                second,
                millisecond,
            )
        } else {
            self.0 = None
        }
    }

    /// `Date.prototype.setUTCMilliseconds()`
    ///
    /// The `setUTCMilliseconds()` method sets the milliseconds for a specified date according to universal time.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.setutcmilliseconds
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/setUTCMilliseconds
    pub fn set_utc_milliseconds(&mut self, millisecond: Option<f64>) {
        if let Some(millisecond) = millisecond {
            self.set_components(true, None, None, None, None, None, None, Some(millisecond))
        } else {
            self.0 = None
        }
    }

    /// `Date.prototype.setUTCMinutes()`
    ///
    /// The `setUTCMinutes()` method sets the minutes for a specified date according to universal time.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.setutcminutes
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/setUTCMinutes
    pub fn set_utc_minutes(
        &mut self,
        minute: Option<f64>,
        second: Option<f64>,
        millisecond: Option<f64>,
    ) {
        if let Some(minute) = minute {
            self.set_components(
                true,
                None,
                None,
                None,
                None,
                Some(minute),
                second,
                millisecond,
            )
        } else {
            self.0 = None
        }
    }

    /// `Date.prototype.setUTCMonth()`
    ///
    /// The `setUTCMonth()` method sets the month for a specified date according to universal time.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.setutcmonth
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/setUTCMonth
    pub fn set_utc_month(&mut self, month: Option<f64>, day: Option<f64>) {
        if let Some(month) = month {
            self.set_components(true, None, Some(month), day, None, None, None, None)
        } else {
            self.0 = None
        }
    }

    /// `Date.prototype.setUTCSeconds()`
    ///
    /// The `setUTCSeconds()` method sets the seconds for a specified date according to universal time.
    ///
    /// More information:
    ///  - [ECMAScript reference][spec]
    ///  - [MDN documentation][mdn]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-date.prototype.setutcseconds
    /// [mdn]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/setUTCSeconds
    pub fn set_utc_seconds(&mut self, second: Option<f64>, millisecond: Option<f64>) {
        if let Some(second) = second {
            self.set_components(
                true,
                None,
                None,
                None,
                None,
                None,
                Some(second),
                millisecond,
            )
        } else {
            self.0 = None
        }
    }
}
impl JsClass for Date {
    fn class() -> &'static Class {
        Self::get_class()
    }
}

pub fn date_constructor(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if !args.ctor_call {
        return Ok(Date::make_date_string(ctx));
    } else {
        let structure = ctx.global_data().date_structure.unwrap();
        let mut object = JsObject::new(ctx, &structure, Date::get_class(), ObjectTag::Ordinary);
        if args.size() == 0 {
            return Ok(Date::make_date_now(ctx, object));
        } else if args.size() == 1 {
            return Date::make_date_single(ctx, object, args.at(1));
        } else {
            return Date::make_date_multiple(ctx, object, args);
        }
    }
}

pub fn date_parse(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    if args.size() == 0 {
        return Ok(JsValue::encode_nan_value());
    }
    match DateTime::parse_from_rfc3339(&args.at(0).to_string(ctx)?) {
        Ok(v) => Ok(JsValue::new(v.naive_utc().timestamp_millis() as f64)),
        _ => Ok(JsValue::encode_nan_value()),
    }
}

pub fn date_utc(context: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let year = args
        .try_at(0)
        .map_or(Ok(f64::NAN), |value| value.to_number(context))?;
    let month = args
        .try_at(1)
        .map_or(Ok(1f64), |value| value.to_number(context))?;
    let day = args
        .try_at(2)
        .map_or(Ok(1f64), |value| value.to_number(context))?;
    let hour = args
        .try_at(3)
        .map_or(Ok(0f64), |value| value.to_number(context))?;
    let min = args
        .try_at(4)
        .map_or(Ok(0f64), |value| value.to_number(context))?;
    let sec = args
        .try_at(5)
        .map_or(Ok(0f64), |value| value.to_number(context))?;
    let milli = args
        .try_at(6)
        .map_or(Ok(0f64), |value| value.to_number(context))?;

    if !check_normal_opt!(year, month, day, hour, min, sec, milli) {
        return Ok(JsValue::encode_nan_value());
    }

    let year = year as i32;
    let month = month as u32;
    let day = day as u32;
    let hour = hour as u32;
    let min = min as u32;
    let sec = sec as u32;
    let milli = milli as u32;

    let year = if (0..=99).contains(&year) {
        1900 + year
    } else {
        year
    };
    NaiveDate::from_ymd_opt(year, month + 1, day)
        .and_then(|f| f.and_hms_milli_opt(hour, min, sec, milli))
        .and_then(|f| Date::time_clip(f.timestamp_millis() as f64))
        .map_or(Ok(JsValue::new(f64::NAN)), |time| Ok(JsValue::new(time)))
}

/// The abstract operation `thisTimeValue` takes argument value.
///
/// In following descriptions of functions that are properties of the Date prototype object, the phrase “this
/// Date object” refers to the object that is the this value for the invocation of the function. If the `Type` of
/// the this value is not `Object`, a `TypeError` exception is thrown. The phrase “this time value” within the
/// specification of a method refers to the result returned by calling the abstract operation `thisTimeValue` with
/// the this value of the method invocation passed as the argument.
///
/// More information:
///  - [ECMAScript reference][spec]
///
/// [spec]: https://tc39.es/ecma262/#sec-thistimevalue
#[inline]

fn this_time_value(value: JsValue, ctx: GcPointer<Context>) -> Result<Date, JsValue> {
    if value.is_jsobject() {
        let object = value.get_jsobject();
        if object.is_class(Date::get_class()) {
            return Ok(**object.data::<Date>());
        }
    }
    Err(JsValue::new(ctx.new_type_error("'this' is not a Date")))
}
pub fn date_to_string(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
    let date = TypedJsObject::<Date>::try_from(ctx, args.this)?;
    Ok(JsValue::new(JsString::new(ctx, (*date).to_string())))
}

macro_rules! getter_method {
    ($n: ident $name:ident) => {
        pub fn $n(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue, JsValue> {
            Ok(JsValue::js_from(
                ctx,
                this_time_value(args.this, ctx)?.$name(),
            ))
        }
    };
    ($n : ident Self::$name:ident) => {
        pub fn $n(_ctx: GcPointer<Context>, _args: &Arguments) -> Result<JsValue, JsValue> {
            Ok(JsValue::js_from(ctx, Date::$name()))
        }
    };
}

macro_rules! setter_method {
    ($new_name : ident $name:ident($($e:expr),* $(,)?)) => {
        pub fn $new_name(ctx: GcPointer<Context>, args: &Arguments) -> Result<JsValue,JsValue> {
            let mut result = this_time_value(args.this, ctx)?;
            result.$name(
                $(
                    args
                        .try_at($e)
                        .and_then(|value| {
                            value.to_numeric_number(ctx).map_or_else(
                                |_| None,
                                |value| {
                                    if value == 0f64 || value.is_normal() {
                                        Some(value)
                                    } else {
                                        None
                                    }
                                },
                            )
                        })
                ),*
            );

            *TypedJsObject::<Date>::try_from(ctx,args.this)? = result;
            //this.set_data(ObjectData::Date(result));
            Ok(JsValue::from(result.get_time()))
        }

    };
}

setter_method!(date_set_date set_date(0));
setter_method!(date_set_full_year set_full_year(0,1,2));
setter_method!(date_set_hours set_hours(0,1,2,3));
setter_method!(date_set_milliseconds set_milliseconds(0));
setter_method!(date_set_minutes set_minutes(0,1,2));
setter_method!(date_set_month set_month(0,1));
setter_method!(date_set_seconds set_seconds(0,1));
setter_method!(date_set_year set_year(0,1,2));
setter_method!(date_set_time set_time(0));
setter_method!(date_set_utc_date set_utc_date(0));
setter_method!(date_set_utc_full_year set_utc_full_year(0,1,2));
setter_method!(date_set_utc_hours set_utc_hours(0,1,2,3));
setter_method!(date_set_utc_minutes set_utc_minutes(0,1,2));
setter_method!(date_set_utc_month set_utc_month(0,1));
setter_method!(date_set_utc_seconds set_utc_seconds(0,1));
getter_method!(date_get_date get_date);
getter_method!(date_get_day get_day);
getter_method!(date_get_full_year get_full_year);
getter_method!(date_get_hours get_hours);
getter_method!(date_get_milliseconds get_milliseconds);
getter_method!(date_get_minutes get_minutes);
getter_method!(date_get_month get_month);
getter_method!(date_get_seconds get_seconds);
getter_method!(date_get_time get_time);
getter_method!(date_get_year get_year);
getter_method!(date_get_utc_date get_utc_date);
getter_method!(date_get_utc_day get_utc_day);
getter_method!(date_get_utc_full_year get_utc_full_year);
getter_method!(date_get_utc_hours get_utc_hours);
getter_method!(date_get_utc_minutes get_utc_minutes);
getter_method!(date_get_utc_milliseconds get_utc_milliseconds);
getter_method!(date_get_utc_month get_utc_month);
getter_method!(date_get_utc_seconds get_utc_seconds);
getter_method!(date_to_json to_json);
getter_method!(date_to_time_string to_time_string);
getter_method!(date_value_of value_of);
getter_method!(date_to_gmt_string to_gmt_string);
getter_method!(date_to_iso_string to_iso_string);
getter_method!(date_to_utc_string to_utc_string);
getter_method!(date_to_date_string to_date_string);

pub fn date_now(_ctx: GcPointer<Context>, _args: &Arguments) -> Result<JsValue, JsValue> {
    Ok(JsValue::new(Utc::now().timestamp_millis() as f64))
}
impl GcPointer<Context> {
    pub(crate) fn init_date_in_global_object(mut self) {
        let mut ctx = self;
        let mut init = || -> Result<(), JsValue> {
            let mut proto = ctx.global_data().date_prototype.unwrap();
            let ctor = proto.get(ctx, "constructor".intern())?;
            self.global_object()
                .put(ctx, "Date".intern(), ctor, false)?;
            Ok(())
        };
        init().unwrap_or_else(|_| unreachable!());
    }
    pub(crate) fn init_date_in_global_data(mut self) {
        let mut ctx = self;
        let mut init = || -> Result<(), JsValue> {
            let obj_proto = self.global_data().object_prototype.unwrap();
            let structure = Structure::new_unique_with_proto(ctx, Some(obj_proto), false);
            let mut proto = JsObject::new(ctx, &structure, Date::get_class(), ObjectTag::Ordinary);
            *proto.data::<Date>() = ManuallyDrop::new(Date(None));

            let date_map = Structure::new_indexed(ctx, Some(proto), false);
            self.global_data.date_structure = Some(date_map);
            let mut ctor = JsNativeFunction::new(ctx, "Date".intern(), date_constructor, 0);
            proto.define_own_property(
                self,
                "constructor".intern(),
                &*DataDescriptor::new(JsValue::new(ctor), C | W),
                false,
            )?;
            let fun = JsNativeFunction::new(ctx, "toString".intern(), date_to_string, 0);
            proto.put(self, "toString".intern(), JsValue::new(fun), false)?;
            def_native_method!(ctx, proto, valueOf, date_value_of, 0)?;
            def_native_method!(ctx, proto, setDate, date_set_date, 1)?;
            def_native_method!(ctx, proto, setFullYear, date_set_full_year, 3)?;
            def_native_method!(ctx, proto, setHours, date_set_hours, 4)?;
            def_native_method!(ctx, proto, setMilliseconds, date_set_milliseconds, 1)?;
            def_native_method!(ctx, proto, setMinutes, date_set_minutes, 3)?;
            def_native_method!(ctx, proto, setMonth, date_set_month, 2)?;
            def_native_method!(ctx, proto, setSeconds, date_set_seconds, 2)?;
            def_native_method!(ctx, proto, setYear, date_set_year, 3)?;
            def_native_method!(ctx, proto, setTime, date_set_time, 1)?;
            def_native_method!(ctx, ctor, now, date_now, 0)?;
            def_native_method!(ctx, ctor, parse, date_parse, 1)?;
            def_native_method!(ctx, ctor, UTC, date_utc, 6)?;
            ctor.define_own_property(
                self,
                "prototype".intern(),
                &*DataDescriptor::new(JsValue::new(proto), NONE),
                false,
            )?;
            self.global_data.date_prototype = Some(proto);

            Ok(())
        };
        init().unwrap_or_else(|_| unreachable!());
    }
}
