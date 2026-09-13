//! A Rust crate for working with financial tenors.
//!
//! Tenors are the time periods commodity and financial contracts are quoted
//! in: half-months, months, quarters, half-years and years. This crate models
//! them as a type plus an integer offset from a 1970 epoch, parses and prints
//! the usual contract-code notations, converts between granularities, and maps
//! tenors onto calendar dates.
//!
//! # Examples
//!
//! ```
//! use tenors::{Tenor, TenorType, TenorDuration};
//!
//! let jan_2024 = Tenor::month_from_ym0(2024, 0);
//! assert_eq!(jan_2024, "F24".parse().unwrap());
//! assert_eq!(jan_2024.to_string(), "F24");
//!
//! // add a quarter to a month
//! let apr_2024 = (jan_2024 + TenorDuration::new(TenorType::Quarter)).unwrap();
//! assert_eq!(apr_2024.to_string(), "J24");
//!
//! // calendar boundaries
//! assert_eq!(apr_2024.last_day().unwrap().to_string(), "2024-04-30");
//! ```
//!
//! # Notation
//!
//! | type      | prints as | also parses                                   |
//! |-----------|-----------|-----------------------------------------------|
//! | HalfMonth | `1HF24`   | `2HF2024`                                     |
//! | Month     | `F24`     | `F7` (current decade), `F2024`                |
//! | Quarter   | `1Q24`    | `24Q1`, `1Q2024`, `2024Q1`                    |
//! | HalfYear  | `1H24`    | `24H1`, `1H2024`, `2024H1`                    |
//! | Year      | `2024`    | `C24`, `Cal24`, `C2024`, `Cal2024`            |
//!
//! Two-digit years resolve against the current century and one-digit years
//! (month codes only) against the current decade — the clock rule — unless
//! you parse with [`Tenor::parse_with_reference`]. Tenors outside the current
//! century print a four-digit year (`F1999`) so that every value round-trips.
//!
//! Half-months split the way Platts' half-month cycles do: the 1st to the
//! 15th is the first half, the 16th to the end of the month the second.

#![warn(missing_docs)]

pub mod interpolation;

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;
use std::ops::{Add, AddAssign, Mul, Sub, SubAssign};
use std::str::FromStr;
use thiserror::Error;

/// Errors that can occur when working with tenors.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum TenorsError {
    /// A duration of one type cannot be added to a tenor of another.
    #[error("cannot add a {duration:?} duration to a {tenor:?} tenor")]
    IncompatibleTenors {
        /// The tenor's type.
        tenor: TenorType,
        /// The duration's type.
        duration: TenorType,
    },
    /// The string is not a tenor in any supported notation.
    #[error("cannot parse {0:?} as a tenor")]
    UnparsableTenorString(String),
    /// The arithmetic would overflow the offset.
    #[error("arithmetic overflow in tenor calculation")]
    ArithmeticOverflow,
}

/// Standard futures month codes, January to December.
pub const FUTURESCODES: [char; 12] = ['F', 'G', 'H', 'J', 'K', 'M', 'N', 'Q', 'U', 'V', 'X', 'Z'];

/// Map a futures month code to its zero-based month.
pub fn futures_code_month0(x: char) -> Option<usize> {
    FUTURESCODES.iter().position(|&c| c == x)
}

/// Tenors are encoded as offsets from this epoch year.
pub const TENOR_EPOCH: isize = 1970;

/// The granularities a tenor can have, finest first.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Deserialize, Serialize, Hash)]
pub enum TenorType {
    /// Half-month: the 1st–15th or the 16th–end of a month.
    HalfMonth,
    /// Calendar month.
    Month,
    /// Three-month quarter.
    Quarter,
    /// Six-month half-year.
    HalfYear,
    /// Calendar year.
    Year,
}

impl TenorType {
    /// Length of one tenor of this type in half-months, the finest unit.
    pub fn granularity(&self) -> isize {
        match self {
            TenorType::HalfMonth => 1,
            TenorType::Month => 2,
            TenorType::Quarter => 6,
            TenorType::HalfYear => 12,
            TenorType::Year => 24,
        }
    }

    /// All types, finest to coarsest.
    pub fn iterator() -> impl Iterator<Item = TenorType> {
        [
            TenorType::HalfMonth,
            TenorType::Month,
            TenorType::Quarter,
            TenorType::HalfYear,
            TenorType::Year,
        ]
        .into_iter()
    }

    /// How many of this type make up a year.
    fn per_year(self) -> isize {
        TenorType::Year.granularity() / self.granularity()
    }
}

/// A specific period in time: a type and an offset from [`TENOR_EPOCH`] in
/// units of that type.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct Tenor {
    tenor_type: TenorType,
    epoch_offset: isize,
}

impl PartialOrd for Tenor {
    /// Only tenors of the same type are comparable; see
    /// [`Tenor::cmp_by_granularity`] for a total order.
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.tenor_type != other.tenor_type {
            return None;
        }
        Some(self.epoch_offset.cmp(&other.epoch_offset))
    }
}

// ---------------------------------------------------------------------------
// construction
// ---------------------------------------------------------------------------

impl Tenor {
    /// A tenor from its type and raw epoch offset. Unchecked.
    pub fn new(tenor_type: TenorType, epoch_offset: isize) -> Tenor {
        Tenor {
            tenor_type,
            epoch_offset,
        }
    }

    /// The offset of `year` from the epoch in units of `ty`, plus `part`;
    /// out-of-range parts carry.
    fn from_parts(ty: TenorType, year: isize, part: isize) -> Tenor {
        let offset = (year - TENOR_EPOCH)
            .checked_mul(ty.per_year())
            .and_then(|o| o.checked_add(part))
            .expect("tenor offset overflow");
        Tenor::new(ty, offset)
    }

    /// The calendar year `year`.
    pub fn year_from_y(year: isize) -> Tenor {
        Tenor::from_parts(TenorType::Year, year, 0)
    }

    /// Half-year `half0` (0 or 1) of `year`; other values carry.
    pub fn half_year_from_yh0(year: isize, half0: isize) -> Tenor {
        Tenor::from_parts(TenorType::HalfYear, year, half0)
    }

    /// Quarter `quarter0` (0–3) of `year`; other values carry, so
    /// `quarter_from_yq0(2024, 4)` is the first quarter of 2025.
    pub fn quarter_from_yq0(year: isize, quarter0: isize) -> Tenor {
        Tenor::from_parts(TenorType::Quarter, year, quarter0)
    }

    /// Month `month0` (0 = January … 11 = December) of `year`; other values
    /// carry, so `month_from_ym0(2024, 12)` is January 2025.
    pub fn month_from_ym0(year: isize, month0: isize) -> Tenor {
        Tenor::from_parts(TenorType::Month, year, month0)
    }

    /// Half-month `half0` (0 = 1st–15th, 1 = 16th–end) of month `month0` of
    /// `year`; out-of-range values carry.
    pub fn half_month_from_ymh0(year: isize, month0: isize, half0: isize) -> Tenor {
        Tenor::from_parts(TenorType::HalfMonth, year, month0 * 2 + half0)
    }

    /// Like [`Tenor::half_year_from_yh0`], but `None` unless `half0` is 0 or 1.
    pub fn try_half_year_from_yh0(year: isize, half0: isize) -> Option<Tenor> {
        (0..2)
            .contains(&half0)
            .then(|| Tenor::half_year_from_yh0(year, half0))
    }

    /// Like [`Tenor::quarter_from_yq0`], but `None` unless `quarter0` is 0–3.
    pub fn try_quarter_from_yq0(year: isize, quarter0: isize) -> Option<Tenor> {
        (0..4)
            .contains(&quarter0)
            .then(|| Tenor::quarter_from_yq0(year, quarter0))
    }

    /// Like [`Tenor::month_from_ym0`], but `None` unless `month0` is 0–11.
    pub fn try_month_from_ym0(year: isize, month0: isize) -> Option<Tenor> {
        (0..12)
            .contains(&month0)
            .then(|| Tenor::month_from_ym0(year, month0))
    }

    /// The half-month containing a date: the 1st–15th is the first half,
    /// the 16th onwards the second.
    pub fn half_month_from_date<T: Datelike>(date: &T) -> Tenor {
        let half0 = if date.day() > 15 { 1 } else { 0 };
        Tenor::half_month_from_ymh0(date.year() as isize, date.month0() as isize, half0)
    }
}

impl<T: Datelike> From<&T> for Tenor {
    /// The half-month containing the date; see [`Tenor::half_month_from_date`].
    fn from(date: &T) -> Tenor {
        Tenor::half_month_from_date(date)
    }
}

// ---------------------------------------------------------------------------
// accessors, ordering, arithmetic
// ---------------------------------------------------------------------------

impl Tenor {
    /// The tenor's type.
    pub fn tenor_type(&self) -> TenorType {
        self.tenor_type
    }

    /// The tenor's type.
    #[deprecated(since = "0.2.0", note = "renamed to `tenor_type()`")]
    pub fn get_tenor_type(&self) -> TenorType {
        self.tenor_type
    }

    /// The raw offset from [`TENOR_EPOCH`] in units of the tenor's type.
    pub fn epoch_offset(&self) -> isize {
        self.epoch_offset
    }

    /// Order by type (finest first), then by time — a total order for mixed vectors.
    ///
    /// ```
    /// use tenors::{Tenor, TenorType};
    /// let mut v = vec![Tenor::quarter_from_yq0(2024, 1), Tenor::month_from_ym0(2024, 0), Tenor::month_from_ym0(2023, 11)];
    /// v.sort_by(Tenor::cmp_by_granularity);
    /// assert_eq!(v.iter().map(ToString::to_string).collect::<Vec<_>>(), ["Z23", "F24", "2Q24"]);
    /// ```
    pub fn cmp_by_granularity(&self, other: &Self) -> Ordering {
        match self.tenor_type.cmp(&other.tenor_type) {
            Ordering::Equal => self.epoch_offset.cmp(&other.epoch_offset),
            ord => ord,
        }
    }

    /// Checked addition of an offset; `None` on overflow.
    pub fn checked_add(&self, offset: isize) -> Option<Tenor> {
        self.epoch_offset
            .checked_add(offset)
            .map(|o| Tenor::new(self.tenor_type, o))
    }

    /// Checked subtraction of an offset; `None` on overflow.
    pub fn checked_sub(&self, offset: isize) -> Option<Tenor> {
        self.epoch_offset
            .checked_sub(offset)
            .map(|o| Tenor::new(self.tenor_type, o))
    }

    /// Saturating addition of an offset.
    pub fn saturating_add(&self, offset: isize) -> Tenor {
        Tenor::new(self.tenor_type, self.epoch_offset.saturating_add(offset))
    }

    /// Saturating subtraction of an offset.
    pub fn saturating_sub(&self, offset: isize) -> Tenor {
        Tenor::new(self.tenor_type, self.epoch_offset.saturating_sub(offset))
    }

    /// The calendar year the tenor starts in.
    pub fn year(&self) -> isize {
        TENOR_EPOCH + self.epoch_offset.div_euclid(self.tenor_type.per_year())
    }

    /// Zero-based position within the year (month 0–11, quarter 0–3, …).
    fn part_of_year(&self) -> isize {
        self.epoch_offset.rem_euclid(self.tenor_type.per_year())
    }

    /// Zero-based month (0 = January) for half-months and months; `None` for coarser types.
    pub fn month0(&self) -> Option<usize> {
        self.part0(TenorType::Month)
    }

    /// Zero-based quarter (0 = Q1) for half-months, months and quarters; `None` for coarser types.
    pub fn quarter0(&self) -> Option<usize> {
        self.part0(TenorType::Quarter)
    }

    /// Zero-based half-year (0 = H1) for anything finer than a year; `None` for years.
    pub fn half_year0(&self) -> Option<usize> {
        self.part0(TenorType::HalfYear)
    }

    /// For a half-month, 0 for the 1st–15th and 1 for the 16th–end; `None` otherwise.
    pub fn half0(&self) -> Option<usize> {
        (self.tenor_type == TenorType::HalfMonth).then(|| self.epoch_offset.rem_euclid(2) as usize)
    }

    /// Position of this tenor within its covering `unit`, if the unit is coarser or equal.
    fn part0(&self, unit: TenorType) -> Option<usize> {
        if unit.granularity() < self.tenor_type.granularity() {
            return None;
        }
        let per_unit = unit.granularity() / self.tenor_type.granularity();
        Some(self.part_of_year().div_euclid(per_unit) as usize)
    }

    /// The coarser tenor of type `target` containing this one; `None` if `target`
    /// is finer than this tenor's type, or on overflow.
    ///
    /// ```
    /// use tenors::{Tenor, TenorType};
    /// let dec_1969 = Tenor::month_from_ym0(1969, 11);
    /// assert_eq!(dec_1969.cover(TenorType::Year), Some(Tenor::year_from_y(1969)));
    /// ```
    pub fn cover(&self, target: TenorType) -> Option<Tenor> {
        if target.granularity() < self.tenor_type.granularity() {
            return None;
        }
        let half_months = self
            .tenor_type
            .granularity()
            .checked_mul(self.epoch_offset)?;
        Some(Tenor::new(
            target,
            half_months.div_euclid(target.granularity()),
        ))
    }

    /// The finer tenors of type `target` that make up this one, in order;
    /// `None` if `target` is coarser than this tenor's type, or on overflow.
    ///
    /// ```
    /// use tenors::{Tenor, TenorType};
    /// let months = Tenor::year_from_y(2020).partition(TenorType::Month).unwrap();
    /// assert_eq!(months.len(), 12);
    /// assert_eq!(months[0].to_string(), "F20");
    /// ```
    pub fn partition(&self, target: TenorType) -> Option<Vec<Tenor>> {
        if target.granularity() > self.tenor_type.granularity() {
            return None;
        }
        let ratio = self.tenor_type.granularity() / target.granularity();
        let base = self.epoch_offset.checked_mul(ratio)?;
        base.checked_add(ratio)?;
        Some((0..ratio).map(|i| Tenor::new(target, base + i)).collect())
    }

    /// The first calendar day of the tenor; `None` only if the year is outside chrono's range.
    pub fn first_day(&self) -> Option<NaiveDate> {
        let hm = self.partition(TenorType::HalfMonth)?[0];
        let (year, month) = hm.calendar_year_month()?;
        NaiveDate::from_ymd_opt(year, month, if hm.half0() == Some(0) { 1 } else { 16 })
    }

    /// The last calendar day of the tenor; `None` only if the year is outside chrono's range.
    pub fn last_day(&self) -> Option<NaiveDate> {
        let hm = *self.partition(TenorType::HalfMonth)?.last()?;
        let (year, month) = hm.calendar_year_month()?;
        if hm.half0() == Some(0) {
            NaiveDate::from_ymd_opt(year, month, 15)
        } else {
            // the day before the 1st of the next month
            let next = hm.cover(TenorType::Month)?.checked_add(1)?;
            let (ny, nm) = next.calendar_year_month()?;
            NaiveDate::from_ymd_opt(ny, nm, 1)?.pred_opt()
        }
    }

    /// (year, 1-based month) as chrono wants them; `None` if the year does not fit an `i32`.
    fn calendar_year_month(&self) -> Option<(i32, u32)> {
        let year = i32::try_from(self.year()).ok()?;
        let month = self.month0()? as u32 + 1;
        Some((year, month))
    }
}

macro_rules! offset_ops {
    ($t:ty) => {
        impl Add<isize> for $t {
            type Output = Tenor;
            /// Panics on overflow; use `checked_add` when that matters.
            fn add(self, rhs: isize) -> Tenor {
                self.checked_add(rhs).expect("tenor offset overflow")
            }
        }
        impl Sub<isize> for $t {
            type Output = Tenor;
            /// Panics on overflow; use `checked_sub` when that matters.
            fn sub(self, rhs: isize) -> Tenor {
                self.checked_sub(rhs).expect("tenor offset overflow")
            }
        }
        impl Add<TenorDuration> for $t {
            type Output = Result<Tenor, TenorsError>;
            fn add(self, rhs: TenorDuration) -> Result<Tenor, TenorsError> {
                self.shift(rhs, 1)
            }
        }
        impl Sub<TenorDuration> for $t {
            type Output = Result<Tenor, TenorsError>;
            fn sub(self, rhs: TenorDuration) -> Result<Tenor, TenorsError> {
                self.shift(rhs, -1)
            }
        }
    };
}
offset_ops!(Tenor);
offset_ops!(&Tenor);

impl AddAssign<isize> for Tenor {
    fn add_assign(&mut self, rhs: isize) {
        *self = *self + rhs;
    }
}

impl SubAssign<isize> for Tenor {
    fn sub_assign(&mut self, rhs: isize) {
        *self = *self - rhs;
    }
}

impl Tenor {
    /// `self ± duration`, checked. A duration can be added when its type is a
    /// whole multiple of the tenor's type (a quarter to a month, a year to a
    /// half-month), never the other way round.
    fn shift(&self, duration: TenorDuration, sign: isize) -> Result<Tenor, TenorsError> {
        let (tg, dg) = (
            self.tenor_type.granularity(),
            duration.tenor_type.granularity(),
        );
        if dg % tg != 0 {
            return Err(TenorsError::IncompatibleTenors {
                tenor: self.tenor_type,
                duration: duration.tenor_type,
            });
        }
        (dg / tg)
            .checked_mul(duration.duration)
            .and_then(|d| d.checked_mul(sign))
            .and_then(|d| self.epoch_offset.checked_add(d))
            .map(|o| Tenor::new(self.tenor_type, o))
            .ok_or(TenorsError::ArithmeticOverflow)
    }
}

/// A number of periods of one type, e.g. three quarters. Add it to a tenor of
/// the same or a finer type.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct TenorDuration {
    tenor_type: TenorType,
    duration: isize,
}

impl TenorDuration {
    /// One period of `tenor_type`; multiply to get more.
    pub fn new(tenor_type: TenorType) -> TenorDuration {
        TenorDuration {
            tenor_type,
            duration: 1,
        }
    }

    /// `n` periods of `tenor_type`.
    pub fn of(tenor_type: TenorType, n: isize) -> TenorDuration {
        TenorDuration {
            tenor_type,
            duration: n,
        }
    }

    /// The duration's type.
    pub fn tenor_type(&self) -> TenorType {
        self.tenor_type
    }

    /// The number of periods.
    pub fn count(&self) -> isize {
        self.duration
    }
}

impl Add<isize> for TenorDuration {
    type Output = Self;
    /// Panics on overflow.
    fn add(self, rhs: isize) -> Self {
        TenorDuration::of(
            self.tenor_type,
            self.duration
                .checked_add(rhs)
                .expect("tenor duration overflow"),
        )
    }
}

impl Sub<isize> for TenorDuration {
    type Output = Self;
    /// Panics on overflow.
    fn sub(self, rhs: isize) -> Self {
        TenorDuration::of(
            self.tenor_type,
            self.duration
                .checked_sub(rhs)
                .expect("tenor duration overflow"),
        )
    }
}

impl Mul<isize> for TenorDuration {
    type Output = Self;
    /// Panics on overflow.
    fn mul(self, rhs: isize) -> Self {
        TenorDuration::of(
            self.tenor_type,
            self.duration
                .checked_mul(rhs)
                .expect("tenor duration overflow"),
        )
    }
}

// ---------------------------------------------------------------------------
// text: Display, FromStr, the other parsers, serde
// ---------------------------------------------------------------------------

/// The calendar year the clock rule resolves short years against.
pub fn current_reference_year() -> isize {
    chrono::Utc::now().year() as isize
}

/// A year as the notation prints it: two digits inside the reference
/// century, at least four digits otherwise (so `F0999` round-trips).
fn format_year(year: isize, reference: isize) -> String {
    if year.div_euclid(100) == reference.div_euclid(100) {
        format!("{:02}", year.rem_euclid(100))
    } else {
        format!("{year:04}")
    }
}

impl std::fmt::Display for Tenor {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let year = format_year(self.year(), current_reference_year());
        match self.tenor_type {
            TenorType::HalfMonth => {
                write!(
                    f,
                    "{}H{}{}",
                    self.half0().unwrap_or(0) + 1,
                    FUTURESCODES[self.month0().unwrap_or(0)],
                    year
                )
            }
            TenorType::Month => write!(f, "{}{}", FUTURESCODES[self.month0().unwrap_or(0)], year),
            TenorType::Quarter => write!(f, "{}Q{}", self.quarter0().unwrap_or(0) + 1, year),
            TenorType::HalfYear => write!(f, "{}H{}", self.half_year0().unwrap_or(0) + 1, year),
            TenorType::Year => write!(f, "{:04}", self.year()),
        }
    }
}

impl FromStr for Tenor {
    type Err = TenorsError;

    /// Parse any supported notation, resolving short years with the clock rule.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_tenor(s, current_reference_year())
    }
}

impl Tenor {
    /// Parse any supported notation, resolving two-digit years against
    /// `reference`'s century and one-digit month codes against its decade.
    ///
    /// ```
    /// use tenors::Tenor;
    /// assert_eq!(Tenor::parse_with_reference("F24", 1950).unwrap(), Tenor::month_from_ym0(1924, 0));
    /// ```
    pub fn parse_with_reference(s: &str, reference: isize) -> Result<Tenor, TenorsError> {
        parse_tenor(s, reference)
    }

    /// Parse a futures month code: `F24`, `F2024`, or `F7` (current decade).
    pub fn month_from_futures_code(fc: &str) -> Result<Tenor, TenorsError> {
        let bad = || TenorsError::UnparsableTenorString(fc.to_string());
        if !fc.is_ascii() {
            return Err(bad());
        }
        let mut chars = fc.chars();
        let month0 = chars.next().and_then(futures_code_month0).ok_or_else(bad)?;
        let year = resolve_year(chars.as_str(), current_reference_year(), true).ok_or_else(bad)?;
        Ok(Tenor::month_from_ym0(year, month0 as isize))
    }
}

/// Parse a CME-style month: three-letter month, a space, two-digit year (`DEC 32`).
pub fn cme_tenor_parser(tenor_string: &str) -> Result<Tenor, TenorsError> {
    let bad = || TenorsError::UnparsableTenorString(tenor_string.to_string());
    if !tenor_string.is_ascii() || tenor_string.len() != 6 {
        return Err(bad());
    }
    let (month, rest) = tenor_string.split_at(3);
    let year_digits = rest.strip_prefix(' ').ok_or_else(bad)?;
    let month0 = cme_month0(month).ok_or_else(bad)?;
    let year = resolve_year(year_digits, current_reference_year(), false).ok_or_else(bad)?;
    Ok(Tenor::month_from_ym0(year, month0 as isize))
}

fn cme_month0(x: &str) -> Option<usize> {
    let months = [
        "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
    ];
    if x == "JLY" {
        return Some(6);
    }
    months.iter().position(|m| m.eq_ignore_ascii_case(x))
}

/// Digits to a year: one digit → `reference`'s decade (if `allow_decade`),
/// two → its century, four or more → literal. Three digits are an error.
fn resolve_year(digits: &str, reference: isize, allow_decade: bool) -> Option<isize> {
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let value: isize = digits.parse().ok()?;
    match digits.len() {
        1 if allow_decade => Some(reference.div_euclid(10) * 10 + value),
        1 | 3 => None,
        2 => Some(reference.div_euclid(100) * 100 + value),
        _ => Some(value),
    }
}

/// The one tokenizer behind every string entry point.
fn parse_tenor(s: &str, reference: isize) -> Result<Tenor, TenorsError> {
    let bad = || TenorsError::UnparsableTenorString(s.to_string());
    if s.is_empty() || !s.is_ascii() {
        return Err(bad());
    }
    let b = s.as_bytes();
    let year = |digits: &str, allow_decade: bool| {
        resolve_year(digits, reference, allow_decade).ok_or_else(bad)
    };

    // years: "C24", "Cal2024", or a bare year of at least four digits
    if let Some(rest) = s.strip_prefix('C') {
        let digits = match rest.get(..2) {
            Some(al) if al.eq_ignore_ascii_case("al") => &rest[2..],
            _ => rest,
        };
        return Ok(Tenor::year_from_y(year(digits, false)?));
    }
    if b.len() >= 4 && b.iter().all(u8::is_ascii_digit) {
        return Ok(Tenor::year_from_y(year(s, false)?));
    }

    // half-months: "1HF24", "2HF2024" — a period digit, 'H', a month letter
    if b.len() >= 5
        && matches!(b[0], b'1' | b'2')
        && b[1] == b'H'
        && let Some(month0) = futures_code_month0(b[2] as char)
    {
        let half0 = (b[0] - b'1') as isize;
        return Ok(Tenor::half_month_from_ymh0(
            year(&s[3..], false)?,
            month0 as isize,
            half0,
        ));
    }

    // month codes: "F7", "F24", "F2024"
    if let Some(month0) = futures_code_month0(b[0] as char) {
        return Ok(Tenor::month_from_ym0(year(&s[1..], true)?, month0 as isize));
    }

    // quarters and half-years, period-first ("1Q24") or year-first ("24Q1")
    for (sep, ty, count) in [('Q', TenorType::Quarter, 4), ('H', TenorType::HalfYear, 2)] {
        if let Some(pos) = s.find(sep) {
            let (before, after) = (&s[..pos], &s[pos + 1..]);
            let (period, year_digits) = if before.len() == 1 {
                (before, after)
            } else {
                (after, before)
            };
            let period: isize = period.parse().map_err(|_| bad())?;
            if !(1..=count).contains(&period) {
                return Err(bad());
            }
            return Ok(Tenor::from_parts(ty, year(year_digits, false)?, period - 1));
        }
    }

    Err(bad())
}

impl Serialize for Tenor {
    /// The canonical string, e.g. `"F24"`.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Tenor {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn century() -> isize {
        current_reference_year().div_euclid(100) * 100
    }
    fn decade() -> isize {
        current_reference_year().div_euclid(10) * 10
    }
    fn t(s: &str) -> Tenor {
        s.parse().unwrap_or_else(|e| panic!("{s}: {e}"))
    }
    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    // --- model ---------------------------------------------------------------

    #[test]
    fn tenor_type_order_and_granularity() {
        assert!(TenorType::HalfMonth < TenorType::Month && TenorType::HalfYear < TenorType::Year);
        assert_eq!(
            TenorType::iterator()
                .map(|t| t.granularity())
                .collect::<Vec<_>>(),
            [1, 2, 6, 12, 24]
        );
    }

    #[test]
    fn constructors_carry_out_of_range_parts() {
        assert_eq!(
            Tenor::month_from_ym0(2024, 12),
            Tenor::month_from_ym0(2025, 0)
        );
        assert_eq!(
            Tenor::month_from_ym0(2024, -1),
            Tenor::month_from_ym0(2023, 11)
        );
        assert_eq!(
            Tenor::quarter_from_yq0(2024, 4),
            Tenor::quarter_from_yq0(2025, 0)
        );
        assert_eq!(
            Tenor::quarter_from_yq0(1969, -1),
            Tenor::quarter_from_yq0(1968, 3)
        );
        assert_eq!(
            Tenor::half_year_from_yh0(2024, 2),
            Tenor::half_year_from_yh0(2025, 0)
        );
        assert_eq!(
            Tenor::half_month_from_ymh0(2024, 11, 2),
            Tenor::half_month_from_ymh0(2025, 0, 0)
        );
    }

    #[test]
    fn try_constructors_reject_out_of_range_parts() {
        assert_eq!(
            Tenor::try_month_from_ym0(2024, 11),
            Some(Tenor::month_from_ym0(2024, 11))
        );
        assert_eq!(Tenor::try_month_from_ym0(2024, 12), None);
        assert_eq!(Tenor::try_month_from_ym0(2024, -1), None);
        assert_eq!(Tenor::try_quarter_from_yq0(2024, 4), None);
        assert_eq!(
            Tenor::try_half_year_from_yh0(2024, 1),
            Some(Tenor::half_year_from_yh0(2024, 1))
        );
        assert_eq!(Tenor::try_half_year_from_yh0(2024, 2), None);
    }

    #[test]
    fn accessors_before_and_after_the_epoch() {
        for (tenor, year, m0, q0, h0) in [
            (
                Tenor::month_from_ym0(2024, 0),
                2024,
                Some(0),
                Some(0),
                Some(0),
            ),
            (
                Tenor::month_from_ym0(2024, 11),
                2024,
                Some(11),
                Some(3),
                Some(1),
            ),
            (
                Tenor::month_from_ym0(1969, 11),
                1969,
                Some(11),
                Some(3),
                Some(1),
            ),
            (
                Tenor::month_from_ym0(1969, 0),
                1969,
                Some(0),
                Some(0),
                Some(0),
            ),
            (
                Tenor::quarter_from_yq0(1968, 3),
                1968,
                None,
                Some(3),
                Some(1),
            ),
            (
                Tenor::half_year_from_yh0(1969, 0),
                1969,
                None,
                None,
                Some(0),
            ),
            (Tenor::year_from_y(1800), 1800, None, None, None),
            (
                Tenor::half_month_from_ymh0(1969, 5, 1),
                1969,
                Some(5),
                Some(1),
                Some(0),
            ),
        ] {
            assert_eq!(tenor.year(), year, "{tenor:?}");
            assert_eq!(tenor.month0(), m0, "{tenor:?}");
            assert_eq!(tenor.quarter0(), q0, "{tenor:?}");
            assert_eq!(tenor.half_year0(), h0, "{tenor:?}");
        }
        assert_eq!(Tenor::half_month_from_ymh0(2024, 0, 1).half0(), Some(1));
        assert_eq!(Tenor::month_from_ym0(2024, 0).half0(), None);
    }

    #[test]
    fn cover_floors_before_the_epoch() {
        let dec69 = Tenor::month_from_ym0(1969, 11);
        assert_eq!(dec69.cover(TenorType::Year), Some(Tenor::year_from_y(1969)));
        assert_eq!(
            dec69.cover(TenorType::Quarter),
            Some(Tenor::quarter_from_yq0(1969, 3))
        );
        assert_eq!(
            dec69.cover(TenorType::HalfYear),
            Some(Tenor::half_year_from_yh0(1969, 1))
        );
        assert_eq!(
            Tenor::month_from_ym0(1970, 0).cover(TenorType::Year),
            Some(Tenor::year_from_y(1970))
        );
        assert_eq!(
            Tenor::half_month_from_ymh0(1969, 0, 0).cover(TenorType::Year),
            Some(Tenor::year_from_y(1969))
        );
        assert_eq!(Tenor::year_from_y(2024).cover(TenorType::Month), None);
        assert_eq!(
            Tenor::month_from_ym0(2024, 4).cover(TenorType::Month),
            Some(Tenor::month_from_ym0(2024, 4))
        );
    }

    #[test]
    fn partition_and_cover_are_inverse() {
        for year in [1800, 1969, 1970, 2024, 2100] {
            let y = Tenor::year_from_y(year);
            for ty in TenorType::iterator() {
                let parts = y.partition(ty).unwrap();
                assert_eq!(parts.len(), ty.per_year() as usize);
                for p in parts {
                    assert_eq!(p.cover(TenorType::Year), Some(y), "{p:?}");
                }
            }
        }
        assert_eq!(
            Tenor::month_from_ym0(2024, 0).partition(TenorType::Year),
            None
        );
    }

    #[test]
    fn first_and_last_day() {
        let jan24 = Tenor::month_from_ym0(2024, 0);
        assert_eq!(jan24.first_day(), Some(date(2024, 1, 1)));
        assert_eq!(jan24.last_day(), Some(date(2024, 1, 31)));
        assert_eq!(
            Tenor::month_from_ym0(2024, 1).last_day(),
            Some(date(2024, 2, 29))
        );
        assert_eq!(
            Tenor::month_from_ym0(2023, 1).last_day(),
            Some(date(2023, 2, 28))
        );
        assert_eq!(
            Tenor::month_from_ym0(2024, 11).last_day(),
            Some(date(2024, 12, 31))
        );
        assert_eq!(
            Tenor::quarter_from_yq0(2024, 1).first_day(),
            Some(date(2024, 4, 1))
        );
        assert_eq!(
            Tenor::quarter_from_yq0(2024, 1).last_day(),
            Some(date(2024, 6, 30))
        );
        assert_eq!(
            Tenor::half_year_from_yh0(2024, 1).last_day(),
            Some(date(2024, 12, 31))
        );
        assert_eq!(Tenor::year_from_y(1969).first_day(), Some(date(1969, 1, 1)));
        assert_eq!(
            Tenor::year_from_y(1969).last_day(),
            Some(date(1969, 12, 31))
        );
        // half-months: 1st–15th, 16th–end
        assert_eq!(
            Tenor::half_month_from_ymh0(2024, 1, 0).first_day(),
            Some(date(2024, 2, 1))
        );
        assert_eq!(
            Tenor::half_month_from_ymh0(2024, 1, 0).last_day(),
            Some(date(2024, 2, 15))
        );
        assert_eq!(
            Tenor::half_month_from_ymh0(2024, 1, 1).first_day(),
            Some(date(2024, 2, 16))
        );
        assert_eq!(
            Tenor::half_month_from_ymh0(2024, 1, 1).last_day(),
            Some(date(2024, 2, 29))
        );
        assert_eq!(
            Tenor::half_month_from_ymh0(2024, 11, 1).last_day(),
            Some(date(2024, 12, 31))
        );
        // outside chrono's range: None, no panic
        assert_eq!(Tenor::year_from_y(300_000).first_day(), None);
        assert_eq!(Tenor::new(TenorType::Month, isize::MAX).last_day(), None);
    }

    #[test]
    fn the_fifteenth_belongs_to_the_first_half() {
        for (y, m, last) in [(2024, 2, 29), (2023, 2, 28), (2024, 4, 30), (2024, 3, 31)] {
            let h1 = Tenor::half_month_from_ymh0(y as isize, m as isize - 1, 0);
            let h2 = Tenor::half_month_from_ymh0(y as isize, m as isize - 1, 1);
            assert_eq!(Tenor::from(&date(y, m, 14)), h1);
            assert_eq!(Tenor::from(&date(y, m, 15)), h1);
            assert_eq!(Tenor::from(&date(y, m, 16)), h2);
            assert_eq!(Tenor::from(&date(y, m, last)), h2);
            assert_eq!(h1.last_day(), Some(date(y, m, 15)));
            assert_eq!(h2.first_day(), Some(date(y, m, 16)));
            assert_eq!(h2.last_day(), Some(date(y, m, last)));
        }
        assert_eq!(
            Tenor::from(&date(1970, 1, 4)),
            Tenor::new(TenorType::HalfMonth, 0)
        );
        assert_eq!(
            Tenor::from(&date(1970, 1, 15)),
            Tenor::new(TenorType::HalfMonth, 0)
        );
        assert_eq!(
            Tenor::from(&date(1970, 1, 16)),
            Tenor::new(TenorType::HalfMonth, 1)
        );
        assert_eq!(
            Tenor::half_month_from_date(&date(1969, 12, 31)),
            Tenor::new(TenorType::HalfMonth, -1)
        );
    }

    // --- arithmetic ------------------------------------------------------------

    #[test]
    fn offset_arithmetic_by_value_and_reference() {
        let m = Tenor::month_from_ym0(2024, 0);
        assert_eq!(m + 1, Tenor::month_from_ym0(2024, 1));
        assert_eq!(&m - 1, Tenor::month_from_ym0(2023, 11));
        let mut n = m;
        n += 12;
        assert_eq!(n, Tenor::month_from_ym0(2025, 0));
        n -= 24;
        assert_eq!(n, Tenor::month_from_ym0(2023, 0));
        for ty in TenorType::iterator() {
            assert_eq!(Tenor::new(ty, 1) + 1, Tenor::new(ty, 2));
            assert_eq!(Tenor::new(ty, 1) - 1, Tenor::new(ty, 0));
        }
    }

    #[test]
    fn checked_and_saturating() {
        let m = Tenor::new(TenorType::Month, 100);
        assert_eq!(m.checked_add(10), Some(Tenor::new(TenorType::Month, 110)));
        assert_eq!(m.checked_add(isize::MAX), None);
        assert_eq!(
            Tenor::new(TenorType::Month, isize::MIN).checked_sub(1),
            None
        );
        assert_eq!(
            m.saturating_add(isize::MAX),
            Tenor::new(TenorType::Month, isize::MAX)
        );
        assert_eq!(
            m.saturating_sub(isize::MAX),
            Tenor::new(TenorType::Month, 100_isize.saturating_sub(isize::MAX))
        );
    }

    #[test]
    #[should_panic(expected = "tenor offset overflow")]
    fn plain_operator_panics_on_overflow() {
        let _ = Tenor::new(TenorType::Month, isize::MAX) + 1;
    }

    #[test]
    fn durations() {
        let m = Tenor::month_from_ym0(2024, 0);
        assert_eq!(
            (m + TenorDuration::new(TenorType::Quarter)).unwrap(),
            Tenor::month_from_ym0(2024, 3)
        );
        assert_eq!(
            (m + TenorDuration::new(TenorType::Year) * 4).unwrap(),
            Tenor::month_from_ym0(2028, 0)
        );
        assert_eq!(
            (m - TenorDuration::of(TenorType::Month, 13)).unwrap(),
            Tenor::month_from_ym0(2022, 11)
        );
        assert_eq!(
            (Tenor::quarter_from_yq0(2024, 0) + TenorDuration::new(TenorType::Month)),
            Err(TenorsError::IncompatibleTenors {
                tenor: TenorType::Quarter,
                duration: TenorType::Month
            })
        );
        assert_eq!(
            Tenor::new(TenorType::Month, isize::MAX) + TenorDuration::new(TenorType::Year),
            Err(TenorsError::ArithmeticOverflow)
        );
        assert_eq!(
            Tenor::new(TenorType::Month, 0) + TenorDuration::of(TenorType::Year, isize::MAX),
            Err(TenorsError::ArithmeticOverflow)
        );
        let d = TenorDuration::new(TenorType::Quarter) + 2 - 1;
        assert_eq!((d.tenor_type(), d.count()), (TenorType::Quarter, 2));
    }

    #[test]
    fn ordering() {
        let m1 = Tenor::month_from_ym0(2024, 0);
        let m2 = Tenor::month_from_ym0(2024, 1);
        assert!(m1 < m2);
        assert_eq!(m1.partial_cmp(&Tenor::quarter_from_yq0(2024, 0)), None);
        let hm1 = Tenor::half_month_from_ymh0(2024, 0, 0);
        let hm2 = Tenor::half_month_from_ymh0(2024, 0, 1);
        let q1 = Tenor::quarter_from_yq0(2024, 0);
        let q2 = Tenor::quarter_from_yq0(2024, 1);
        let h1 = Tenor::half_year_from_yh0(2024, 0);
        let h2 = Tenor::half_year_from_yh0(2024, 1);
        let y1 = Tenor::year_from_y(2024);
        let y2 = Tenor::year_from_y(2025);
        let mut v = vec![y1, q2, hm1, m2, h1, q1, hm2, m1, h2, y2];
        v.sort_by(Tenor::cmp_by_granularity);
        assert_eq!(v, [hm1, hm2, m1, m2, q1, q2, h1, h2, y1, y2]);
    }

    // --- text ------------------------------------------------------------------

    #[test]
    fn display_every_type() {
        let c = century();
        assert_eq!(
            Tenor::half_month_from_ymh0(c + 20, 0, 0).to_string(),
            "1HF20"
        );
        assert_eq!(
            Tenor::half_month_from_ymh0(c + 20, 0, 1).to_string(),
            "2HF20"
        );
        assert_eq!(Tenor::month_from_ym0(c + 24, 0).to_string(), "F24");
        assert_eq!(Tenor::month_from_ym0(c + 24, 11).to_string(), "Z24");
        assert_eq!(Tenor::month_from_ym0(c + 3, 7).to_string(), "Q03");
        assert_eq!(Tenor::quarter_from_yq0(c + 24, 0).to_string(), "1Q24");
        assert_eq!(Tenor::half_year_from_yh0(c + 24, 1).to_string(), "2H24");
        assert_eq!(
            Tenor::year_from_y(c + 24).to_string(),
            format!("{:04}", c + 24)
        );
        // other centuries print the full year, zero-padded to four digits
        assert_eq!(Tenor::year_from_y(999).to_string(), "0999");
        assert_eq!(Tenor::month_from_ym0(1, 0).to_string(), "F0001");
        assert_eq!(Tenor::month_from_ym0(1999, 0).to_string(), "F1999");
        assert_eq!(Tenor::quarter_from_yq0(1969, 3).to_string(), "4Q1969");
        assert_eq!(
            Tenor::half_month_from_ymh0(2200, 5, 1).to_string(),
            "2HM2200"
        );
    }

    #[test]
    fn parse_month_codes() {
        assert_eq!(t("F24"), Tenor::month_from_ym0(century() + 24, 0));
        assert_eq!(t("Z99"), Tenor::month_from_ym0(century() + 99, 11));
        assert_eq!(t("M00"), Tenor::month_from_ym0(century(), 5));
        assert_eq!(t("F7"), Tenor::month_from_ym0(decade() + 7, 0));
        assert_eq!(t("M0"), Tenor::month_from_ym0(decade(), 5));
        assert_eq!(t("F2024"), Tenor::month_from_ym0(2024, 0));
        assert_eq!(t("M1970"), Tenor::month_from_ym0(1970, 5));
        assert_eq!(t("F3000"), Tenor::month_from_ym0(3000, 0));
        // H and Q alone are months (March, August), not half-years/quarters
        assert_eq!(t("H24").tenor_type(), TenorType::Month);
        assert_eq!(t("Q2024"), Tenor::month_from_ym0(2024, 7));
        for bad in [
            "F", "A7", "FA", "A24", "F2A", "FX4", "F202", "A2024", "F202A", "F20245x",
        ] {
            assert!(bad.parse::<Tenor>().is_err(), "{bad}");
        }
        // three-digit years are neither short nor full: an error; four digits are literal
        assert!("F999".parse::<Tenor>().is_err());
        assert_eq!(t("F0999"), Tenor::month_from_ym0(999, 0));
    }

    #[test]
    fn month_from_futures_code_and_cme_use_the_same_year_rule() {
        assert_eq!(Tenor::month_from_futures_code("F24").unwrap(), t("F24"));
        assert_eq!(Tenor::month_from_futures_code("F7").unwrap(), t("F7"));
        assert_eq!(
            Tenor::month_from_futures_code("F2024").unwrap(),
            Tenor::month_from_ym0(2024, 0)
        );
        assert_eq!(
            Tenor::month_from_futures_code("R24"),
            Err(TenorsError::UnparsableTenorString("R24".into()))
        );
        assert_eq!(
            Tenor::month_from_futures_code("F2t"),
            Err(TenorsError::UnparsableTenorString("F2t".into()))
        );
        assert_eq!(
            cme_tenor_parser("DEC 32").unwrap(),
            Tenor::month_from_ym0(century() + 32, 11)
        );
        assert_eq!(
            cme_tenor_parser("JLY 05").unwrap(),
            Tenor::month_from_ym0(century() + 5, 6)
        );
        assert_eq!(cme_tenor_parser("jan 24").unwrap(), t("F24"));
        for bad in ["DEC32", "DEC 3", "XYZ 32", "DEC  2", ""] {
            assert!(cme_tenor_parser(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn parse_quarters_and_half_years() {
        let c = century();
        assert_eq!(t("1Q24"), Tenor::quarter_from_yq0(c + 24, 0));
        assert_eq!(t("4Q27"), Tenor::quarter_from_yq0(c + 27, 3));
        assert_eq!(t("24Q1"), Tenor::quarter_from_yq0(c + 24, 0));
        assert_eq!(t("2025Q4"), Tenor::quarter_from_yq0(2025, 3));
        assert_eq!(t("3Q2026"), Tenor::quarter_from_yq0(2026, 2));
        assert_eq!(t("1H24"), Tenor::half_year_from_yh0(c + 24, 0));
        assert_eq!(t("2H2025"), Tenor::half_year_from_yh0(2025, 1));
        assert_eq!(t("24H1"), Tenor::half_year_from_yh0(c + 24, 0));
        assert_eq!(t("2025H2"), Tenor::half_year_from_yh0(2025, 1));
        for bad in [
            "0Q24", "5Q24", "24Q0", "24Q5", "0H24", "3H24", "24H0", "24H3", "1Q", "Q", "1Qx4",
        ] {
            assert!(bad.parse::<Tenor>().is_err(), "{bad}");
        }
    }

    #[test]
    fn parse_years() {
        let c = century();
        assert_eq!(t("C24"), Tenor::year_from_y(c + 24));
        assert_eq!(t("C00"), Tenor::year_from_y(c));
        assert_eq!(t("C2024"), Tenor::year_from_y(2024));
        assert_eq!(t("C1970"), Tenor::new(TenorType::Year, 0));
        assert_eq!(t("Cal24"), Tenor::year_from_y(c + 24));
        assert_eq!(t("CAL24"), Tenor::year_from_y(c + 24));
        assert_eq!(t("CaL2024"), Tenor::year_from_y(2024));
        assert_eq!(t("2024"), Tenor::year_from_y(2024));
        assert_eq!(t("1969"), Tenor::year_from_y(1969));
        assert_eq!(t("12345"), Tenor::year_from_y(12345));
        assert_eq!(t("0999"), Tenor::year_from_y(999));
        for bad in [
            "C", "C2", "C202", "Cal", "Cal2", "D24", "24", "202", "-2024",
        ] {
            assert!(bad.parse::<Tenor>().is_err(), "{bad}");
        }
    }

    #[test]
    fn parse_half_months() {
        let c = century();
        assert_eq!(t("1HF24"), Tenor::half_month_from_ymh0(c + 24, 0, 0));
        assert_eq!(t("2HF24"), Tenor::half_month_from_ymh0(c + 24, 0, 1));
        assert_eq!(t("2HZ2024"), Tenor::half_month_from_ymh0(2024, 11, 1));
        for bad in ["3HF24", "1HA24", "1HF2", "1HF", "0HF24"] {
            assert!(bad.parse::<Tenor>().is_err(), "{bad}");
        }
    }

    #[test]
    fn parse_with_reference_year() {
        assert_eq!(
            Tenor::parse_with_reference("F24", 1950).unwrap(),
            Tenor::month_from_ym0(1924, 0)
        );
        assert_eq!(
            Tenor::parse_with_reference("F7", 1983).unwrap(),
            Tenor::month_from_ym0(1987, 0)
        );
        assert_eq!(
            Tenor::parse_with_reference("1Q24", 2150).unwrap(),
            Tenor::quarter_from_yq0(2124, 0)
        );
        assert_eq!(
            Tenor::parse_with_reference("F2024", 1950).unwrap(),
            Tenor::month_from_ym0(2024, 0)
        );
    }

    #[test]
    fn non_ascii_input_is_an_error_not_a_panic() {
        for s in ["C€", "€", "ÄÄ32", "F2€", "1HF2€", "1Q2€", "Ä"] {
            assert!(
                matches!(
                    s.parse::<Tenor>(),
                    Err(TenorsError::UnparsableTenorString(_))
                ),
                "{s}"
            );
            assert!(Tenor::month_from_futures_code(s).is_err(), "{s}");
            assert!(cme_tenor_parser(s).is_err(), "{s}");
        }
        assert!("".parse::<Tenor>().is_err());
    }

    #[test]
    fn display_and_parse_round_trip_every_type_and_century() {
        let years = [
            1,
            999,
            1000,
            1800,
            1969,
            1970,
            1999,
            2000,
            century(),
            century() + 24,
            century() + 99,
            2100,
            2200,
            9999,
        ];
        let mut checked = 0;
        for year in years {
            for ty in TenorType::iterator() {
                for part in 0..ty.per_year() {
                    let tenor = Tenor::from_parts(ty, year, part);
                    let text = tenor.to_string();
                    assert_eq!(text.parse::<Tenor>().unwrap(), tenor, "{tenor:?} -> {text}");
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, years.len() * 43); // 24 + 12 + 4 + 2 + 1 tenors per year
    }

    #[test]
    fn serde_is_the_canonical_string() {
        let m = Tenor::month_from_ym0(2024, 0);
        let json = serde_json::to_string(&m).unwrap();
        assert_eq!(json, format!("\"{m}\""));
        assert_eq!(serde_json::from_str::<Tenor>(&json).unwrap(), m);
        let list: Vec<Tenor> =
            serde_json::from_str(r#"["1HF24","F24","1Q24","1H24","2024"]"#).unwrap();
        assert_eq!(
            list.iter().map(|t| t.tenor_type()).collect::<Vec<_>>(),
            [
                TenorType::HalfMonth,
                TenorType::Month,
                TenorType::Quarter,
                TenorType::HalfYear,
                TenorType::Year
            ]
        );
        assert!(serde_json::from_str::<Tenor>("\"nope\"").is_err());
        assert!(
            serde_json::from_str::<Tenor>(r#"{"tenor_type":"Month","epoch_offset":648}"#).is_err()
        );
    }
}
