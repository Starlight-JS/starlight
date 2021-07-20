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
    fn make_date_now(ctx: GcPointer<Context>, object: GcPointer<JsObject>) -> JsValue {
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
