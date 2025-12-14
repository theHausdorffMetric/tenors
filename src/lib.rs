//! A Rust crate for working with financial tenors.
//!
//! Tenors are time periods commonly used in financial contracts, such as
//! months, quarters, half-years, and years. This crate provides types and
//! operations for manipulating these time periods with proper type safety.
//!
//! # Examples
//!
//! ```
//! use tenors::{Tenor, TenorType, TenorDuration};
//!
//! // Create a tenor for January 2024
//! let jan_2024 = Tenor::month_from_ym0(2024, 0);
//!
//! // Add 3 months using TenorDuration
//! let apr_2024 = (&jan_2024 + TenorDuration::new(TenorType::Month) * 3).unwrap();
//!
//! // Parse futures codes
//! let tenor = Tenor::month_from_futures_code("F24").unwrap();
//! ```

#![warn(missing_docs)]

pub mod interpolation;

use chrono::Datelike;
use log::debug;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::ops::{Add, AddAssign, Mul, Sub, SubAssign};
use std::str::FromStr;
use thiserror::Error;

/// Errors that can occur when working with tenors.
#[derive(Error, Debug, Eq, PartialEq)]
pub enum TenorsError {
    /// Attempted to perform an operation between incompatible tenor types.
    #[error("cannot add {0} to {1}")]
    IncompatibleTenors(String, String),
    /// Failed to parse a string into a tenor.
    #[error("cannot parse {0} to tenor")]
    UnparsableTenorString(String),
    /// Arithmetic operation would cause overflow.
    #[error("arithmetic overflow in tenor calculation")]
    ArithmeticOverflow,
}

/// Standard financial futures abbreviations for months
pub const FUTURESCODES: [char; 12] = ['F', 'G', 'H', 'J', 'K', 'M', 'N', 'Q', 'U', 'V', 'X', 'Z'];

/// Function which matches FUTERESCODES to month indexes
pub fn futures_code_month0(x: char) -> Option<usize> {
    match x {
        'F' => Some(0),
        'G' => Some(1),
        'H' => Some(2),
        'J' => Some(3),
        'K' => Some(4),
        'M' => Some(5),
        'N' => Some(6),
        'Q' => Some(7),
        'U' => Some(8),
        'V' => Some(9),
        'X' => Some(10),
        'Z' => Some(11),
        _ => None,
    }
}

/// function to parse 3 letter months abbreviations
/// currently used for CME parsing
/// the return value indicates month as 0 to 11
fn cme_month0(x: &str) -> Option<usize> {
    match x {
        "JAN" => Some(0),
        "FEB" => Some(1),
        "MAR" => Some(2),
        "APR" => Some(3),
        "MAY" => Some(4),
        "JUN" => Some(5),
        "JUL" => Some(6),
        "JLY" => Some(6),
        "AUG" => Some(7),
        "SEP" => Some(8),
        "OCT" => Some(9),
        "NOV" => Some(10),
        "DEC" => Some(11),
        _ => None,
    }
}
/// function to parse a string of 6 chars into a month tenor
pub fn cme_tenor_parser(tenor_string: &str) -> Result<Tenor, TenorsError> {
    if tenor_string.len() != 6 {
        return Err(TenorsError::UnparsableTenorString(tenor_string.to_string()));
    };
    let month: usize = cme_month0(&tenor_string[0..3])
        .ok_or_else(|| TenorsError::UnparsableTenorString(tenor_string.to_string()))?;
    let year: isize = 2000
        + tenor_string[4..6]
            .parse::<isize>()
            .map_err(|_| TenorsError::UnparsableTenorString(tenor_string.to_string()))?;
    let res = Tenor::month_from_ym0(year, month);
    Ok(res)
}

/// Tenors will be coded in terms of offsets to a fixed epoch 1970
pub const TENOR_EPOCH: isize = 1970;

/// An enum, the variants of which list the different types of tenors
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Deserialize, Serialize, Hash)]
pub enum TenorType {
    /// Half-month period (1st-15th or 16th-end of month).
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
    /// The `granularity` method returns the duration of a TenorType in terms of
    /// the most granular tenor, which currently is a `HalfMonth`.
    pub fn granularity(&self) -> isize {
        match self {
            TenorType::HalfMonth => 1,
            TenorType::Month => 2,
            TenorType::Quarter => 6,
            TenorType::HalfYear => 12,
            TenorType::Year => 24,
        }
    }
    /// Returns an iterator over all tenor types from finest to coarsest granularity.
    pub fn iterator() -> impl Iterator<Item = TenorType> {
        [
            TenorType::HalfMonth,
            TenorType::Month,
            TenorType::Quarter,
            TenorType::HalfYear,
            TenorType::Year,
        ]
        .iter()
        .copied()
    }
}

/// A structure describing a specific tenor in time
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, Hash)]
pub struct Tenor {
    /// The type of the tenor
    tenor_type: TenorType,
    /// The specific point in time is encoded as an offset to TENOR_EPOCH
    epoch_offset: isize,
}

impl PartialOrd for Tenor {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.tenor_type != other.tenor_type {
            return None;
        }
        Some(self.epoch_offset.cmp(&other.epoch_offset))
    }
}

impl Tenor {
    /// Compares two tenors by granularity first, then by epoch offset.
    ///
    /// This provides a total ordering suitable for sorting mixed tenor types.
    /// Tenors are ordered by granularity (HalfMonth < Month < Quarter < HalfYear < Year),
    /// and within the same type by their epoch offset.
    ///
    /// # Examples
    ///
    /// ```
    /// use tenors::{Tenor, TenorType};
    ///
    /// let mut tenors = vec![
    ///     Tenor::quarter_from_yq0(2024, 1),
    ///     Tenor::month_from_ym0(2024, 0),
    ///     Tenor::month_from_ym0(2023, 11),
    ///     Tenor::new(TenorType::HalfMonth, 1296),
    /// ];
    /// tenors.sort_by(Tenor::cmp_by_granularity);
    /// // Result: HalfMonth, then months (2023-Dec, 2024-Jan), then quarter
    /// ```
    pub fn cmp_by_granularity(&self, other: &Self) -> Ordering {
        match self.tenor_type.cmp(&other.tenor_type) {
            Ordering::Equal => self.epoch_offset.cmp(&other.epoch_offset),
            ord => ord,
        }
    }
}

impl<T> From<&T> for Tenor
where
    T: Datelike,
{
    fn from(date: &T) -> Tenor {
        let tmp_month = Tenor::month_from_ym0(date.year() as isize, date.month0() as usize);
        let mut eo = tmp_month.epoch_offset * 2;
        if date.day() > 14 {
            eo += 1;
        }
        Tenor {
            tenor_type: TenorType::HalfMonth,
            epoch_offset: eo,
        }
    }
}

impl Tenor {
    /// Returns the type of this tenor.
    pub fn get_tenor_type(&self) -> TenorType {
        self.tenor_type
    }

    /// Checked addition of an offset to a Tenor.
    /// Returns None if overflow would occur.
    ///
    /// # Examples
    ///
    /// ```
    /// use tenors::{Tenor, TenorType};
    ///
    /// let tenor = Tenor::new(TenorType::Month, 100);
    /// assert_eq!(tenor.checked_add(10), Some(Tenor::new(TenorType::Month, 110)));
    /// assert_eq!(tenor.checked_add(isize::MAX), None); // Overflow
    /// ```
    pub fn checked_add(&self, offset: isize) -> Option<Tenor> {
        self.epoch_offset
            .checked_add(offset)
            .map(|new_offset| Tenor {
                tenor_type: self.tenor_type,
                epoch_offset: new_offset,
            })
    }

    /// Checked subtraction of an offset from a Tenor.
    /// Returns None if overflow would occur.
    pub fn checked_sub(&self, offset: isize) -> Option<Tenor> {
        self.epoch_offset
            .checked_sub(offset)
            .map(|new_offset| Tenor {
                tenor_type: self.tenor_type,
                epoch_offset: new_offset,
            })
    }

    /// Saturating addition - clamps at isize::MAX on overflow
    pub fn saturating_add(&self, offset: isize) -> Tenor {
        Tenor {
            tenor_type: self.tenor_type,
            epoch_offset: self.epoch_offset.saturating_add(offset),
        }
    }

    /// Saturating subtraction - clamps at isize::MIN on underflow  
    pub fn saturating_sub(&self, offset: isize) -> Tenor {
        Tenor {
            tenor_type: self.tenor_type,
            epoch_offset: self.epoch_offset.saturating_sub(offset),
        }
    }

    /// Creates a month tenor from year and zero-based month (0=January, 11=December).
    pub fn month_from_ym0(year: isize, month0: usize) -> Tenor {
        let month_checked = (month0 % 12) as isize;
        let ratio = TenorType::Year.granularity() / TenorType::Month.granularity();
        Tenor {
            tenor_type: TenorType::Month,
            // todo error/range checking
            epoch_offset: (year - TENOR_EPOCH) * ratio + (month_checked % ratio),
        }
    }
    /// parse a futures code in a yearmonth
    ///
    /// # Arguments
    /// * `fc` - a reference to a string representing a futures code, either 3 digit F20 or 5 digit F2000
    ///
    /// # Return Value
    /// * `Result` - either the parsed month as a tenor or a parse error
    /// # Examples
    ///
    /// ```
    /// use tenors::Tenor;
    /// let my_month = Tenor::month_from_futures_code("F2024").unwrap();
    /// println!("F2024 is parsed into {:?} resp {}", my_month, my_month);
    /// ```

    pub fn month_from_futures_code(fc: &str) -> Result<Tenor, TenorsError> {
        if fc.len() == 3 {
            if let (Some(month), Ok(year)) = (
                futures_code_month0(fc.chars().next().unwrap()),
                fc[1..3].parse::<isize>(),
            ) {
                return Ok(Tenor::month_from_ym0(year + 2000, month));
            }
        }
        if fc.len() == 5 {
            if let (Some(month), Ok(year)) = (
                futures_code_month0(fc.chars().next().unwrap()),
                fc[1..5].parse::<isize>(),
            ) {
                return Ok(Tenor::month_from_ym0(year, month));
            }
        }
        Err(TenorsError::UnparsableTenorString(fc.to_string()))
    }
    /// Creates a quarter tenor from year and zero-based quarter (0=Q1, 3=Q4).
    pub fn quarter_from_yq0(year: isize, quarter0: isize) -> Tenor {
        let ratio = TenorType::Year.granularity() / TenorType::Quarter.granularity();
        Tenor {
            tenor_type: TenorType::Quarter,
            // todo error/range checking
            epoch_offset: (year - TENOR_EPOCH) * ratio + (quarter0 % ratio),
        }
    }
    /// Returns the year component of this tenor, if applicable to its type.
    pub fn year(&self) -> Option<isize> {
        if TenorType::Year.granularity() < self.tenor_type.granularity() {
            return None;
        }
        let ratio = TenorType::Year.granularity() / self.tenor_type.granularity();
        Some(TENOR_EPOCH + self.epoch_offset.div_euclid(ratio))
    }
    /// Returns the zero-based month (0=January, 11=December) of this tenor, if applicable.
    pub fn month0(&self) -> Option<usize> {
        if TenorType::Month.granularity() < self.tenor_type.granularity() {
            return None;
        }
        let ratio = TenorType::Year.granularity() / self.tenor_type.granularity();
        let rem_y = self.epoch_offset.rem_euclid(ratio);
        let ratio_m = TenorType::Month.granularity() / self.tenor_type.granularity();
        Some(rem_y.div_euclid(ratio_m) as usize)
    }
    /// Returns the zero-based quarter (0=Q1, 3=Q4) of this tenor, if applicable.
    pub fn quarter0(&self) -> Option<usize> {
        if TenorType::Quarter.granularity() < self.tenor_type.granularity() {
            return None;
        }
        let ratio = TenorType::Year.granularity() / self.tenor_type.granularity();
        let rem_y = self.epoch_offset.rem_euclid(ratio);
        let ratio_q = TenorType::Quarter.granularity() / self.tenor_type.granularity();
        Some(rem_y.div_euclid(ratio_q) as usize)
    }
    /// Creates a new tenor with the specified type and epoch offset.
    pub fn new(tenor_type: TenorType, epoch_offset: isize) -> Tenor {
        Tenor {
            tenor_type,
            epoch_offset,
        }
    }
    /// Find the coarser tenor which covers/includes a given tenor.
    ///
    /// # Examples
    ///
    /// ```
    /// use tenors::{Tenor, TenorType};
    ///
    /// let month = Tenor::month_from_ym0(2024, 3); // April 2024
    /// let quarter = month.cover(TenorType::Quarter).unwrap();
    /// let year = month.cover(TenorType::Year).unwrap();
    /// ```
    pub fn cover(&self, target: TenorType) -> Option<Tenor> {
        if target.granularity() < self.tenor_type.granularity() {
            return None;
        }
        Some(Tenor {
            tenor_type: target,
            epoch_offset: self.tenor_type.granularity() * self.epoch_offset / target.granularity(),
        })
    }
    /// Partition a tenor into a more granular TenorType.
    ///
    /// # Examples
    ///
    /// ```
    /// use tenors::{Tenor, TenorType};
    ///
    /// let year = Tenor::new(TenorType::Year, 50); // 2020
    /// let months = year.partition(TenorType::Month).unwrap();
    /// assert_eq!(months.len(), 12);
    /// ```
    pub fn partition(&self, target: TenorType) -> Option<Vec<Tenor>> {
        if target.granularity() > self.tenor_type.granularity() {
            return None;
        }
        let ratio = self.tenor_type.granularity() / target.granularity();
        let base_offset = self.epoch_offset * ratio;
        Some(
            (0..ratio)
                .map(|x| Tenor {
                    tenor_type: target,
                    epoch_offset: base_offset + x,
                })
                .collect::<Vec<Tenor>>(),
        )
    }
    /// Returns the first calendar day covered by this tenor.
    pub fn first_day(&self) -> chrono::NaiveDate {
        match self.tenor_type {
            TenorType::HalfMonth => {
                let ratio = TenorType::Month.granularity() / TenorType::HalfMonth.granularity();
                if self.epoch_offset.rem_euclid(ratio) == 0 {
                    chrono::NaiveDate::from_ymd_opt(
                        self.year().unwrap() as i32,
                        (self.month0().unwrap() + 1) as u32,
                        1,
                    )
                    .unwrap()
                } else {
                    chrono::NaiveDate::from_ymd_opt(
                        self.year().unwrap() as i32,
                        (self.month0().unwrap() + 1) as u32,
                        16,
                    )
                    .unwrap()
                }
            }
            _ => self
                .partition(TenorType::HalfMonth)
                .unwrap()
                .first()
                .unwrap()
                .first_day(),
        }
    }
    /// Returns the last calendar day covered by this tenor.
    pub fn last_day(&self) -> chrono::NaiveDate {
        debug!("last_day called with {:?} : {}", self, self);
        match self.tenor_type {
            TenorType::HalfMonth => {
                let ratio = TenorType::Month.granularity() / TenorType::HalfMonth.granularity();
                if self.epoch_offset.rem_euclid(ratio) == 0 {
                    chrono::NaiveDate::from_ymd_opt(
                        self.year().unwrap() as i32,
                        (self.month0().unwrap() + 1) as u32,
                        15,
                    )
                    .unwrap()
                } else {
                    // shift to next month and deduct one day
                    let tmp =
                        (&Tenor::month_from_ym0(self.year().unwrap(), self.month0().unwrap())
                            + TenorDuration::new(TenorType::Month))
                        .unwrap();
                    debug!("{} shifted to {}", self, tmp);
                    debug!(
                        "{} with year:{} month0:{}",
                        tmp,
                        tmp.year().unwrap(),
                        tmp.month0().unwrap(),
                    );

                    let ret = chrono::NaiveDate::from_ymd_opt(
                        tmp.year().unwrap() as i32,
                        (tmp.month0().unwrap() + 1) as u32,
                        1,
                    )
                    .unwrap();
                    debug!("with day {ret} to be backshifted by one");
                    ret - chrono::Duration::try_days(1).unwrap()
                }
            }
            _ => self
                .partition(TenorType::HalfMonth)
                .unwrap()
                .last()
                .unwrap()
                .last_day(),
        }
    }
}

impl std::fmt::Display for Tenor {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match (self).tenor_type {
            TenorType::HalfMonth => {
                let ratio_y = TenorType::Year.granularity() / TenorType::HalfMonth.granularity();
                let ratio_m = TenorType::Month.granularity() / TenorType::HalfMonth.granularity();
                let rem_y = self.epoch_offset.rem_euclid(ratio_y);
                write!(
                    f,
                    "{}H{}{:02}",
                    rem_y.rem_euclid(ratio_m) as usize + 1,
                    FUTURESCODES[rem_y.div_euclid(ratio_m) as usize],
                    (TENOR_EPOCH + self.epoch_offset.div_euclid(ratio_y)) % 100,
                )
            }
            TenorType::Month => {
                let ratio = TenorType::Year.granularity() / TenorType::Month.granularity();
                // debug!("{ratio}");
                write!(
                    f,
                    "{}{:02}",
                    FUTURESCODES[self.epoch_offset.rem_euclid(ratio) as usize],
                    (TENOR_EPOCH + self.epoch_offset.div_euclid(ratio)) % 100,
                )
            }
            TenorType::Quarter => {
                let ratio = TenorType::Year.granularity() / TenorType::Quarter.granularity();
                write!(
                    f,
                    "{}Q{:02}",
                    self.epoch_offset.rem_euclid(ratio) as usize + 1,
                    (TENOR_EPOCH + self.epoch_offset.div_euclid(ratio)) % 100,
                )
            }
            TenorType::HalfYear => {
                let ratio = TenorType::Year.granularity() / TenorType::HalfYear.granularity();
                write!(
                    f,
                    "{}H{:02}",
                    self.epoch_offset.rem_euclid(ratio) as usize + 1,
                    (TENOR_EPOCH + self.epoch_offset.div_euclid(ratio)) % 100,
                )
            }
            TenorType::Year => {
                write!(f, "{}", TENOR_EPOCH + self.epoch_offset,)
            }
        }
    }
}

impl FromStr for Tenor {
    type Err = TenorsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Check for year format first (starts with 'C')
        if s.starts_with('C') {
            return parse_year_format(s);
        }

        // Try month futures codes (existing formats)
        if let Ok(tenor) = parse_month_futures(s) {
            return Ok(tenor);
        }

        // Try quarter format
        if let Ok(tenor) = parse_quarter_format(s) {
            return Ok(tenor);
        }

        // Try half-year format
        if let Ok(tenor) = parse_halfyear_format(s) {
            return Ok(tenor);
        }

        Err(TenorsError::UnparsableTenorString(s.to_string()))
    }
}

fn parse_month_futures(s: &str) -> Result<Tenor, TenorsError> {
    // Format 1: 2-char futures code (e.g., "F7" -> January 2027)
    if s.len() == 2 {
        let chars: Vec<char> = s.chars().collect();
        if let (Some(month0), Some(digit)) = (futures_code_month0(chars[0]), chars[1].to_digit(10))
        {
            // Derive current decade from system date
            let current_year = chrono::Utc::now().year();
            let current_decade = (current_year / 10) * 10;
            let year = current_decade as isize + digit as isize;
            return Ok(Tenor::month_from_ym0(year, month0));
        }
    }

    // Format 2: 3-char futures code (e.g., "F24" -> January 2024)
    if s.len() == 3 {
        let chars: Vec<char> = s.chars().collect();
        if let Some(month0) = futures_code_month0(chars[0]) {
            // Parse the two-digit year offset
            if let Ok(year_offset) = s[1..3].parse::<isize>() {
                // Derive current century from system date
                let current_year = chrono::Utc::now().year();
                let current_century = (current_year / 100) * 100;
                let year = current_century as isize + year_offset;
                return Ok(Tenor::month_from_ym0(year, month0));
            }
        }
    }

    // Format 3: 5-char futures code with full year (e.g., "F2024" -> January 2024)
    if s.len() == 5 {
        let chars: Vec<char> = s.chars().collect();
        if let Some(month0) = futures_code_month0(chars[0]) {
            // Parse the full 4-digit year
            if let Ok(year) = s[1..5].parse::<isize>() {
                return Ok(Tenor::month_from_ym0(year, month0));
            }
        }
    }

    Err(TenorsError::UnparsableTenorString(s.to_string()))
}

fn parse_year_format(s: &str) -> Result<Tenor, TenorsError> {
    if !s.starts_with('C') {
        return Err(TenorsError::UnparsableTenorString(s.to_string()));
    }

    let remainder = &s[1..];

    // Check for optional "al" (case insensitive)
    let year_part = if remainder.len() >= 2 && remainder[..2].eq_ignore_ascii_case("al") {
        &remainder[2..]
    } else {
        remainder
    };

    // Parse 2-digit (with century offset) or 4-digit year
    let year = match year_part.len() {
        2 => year_part
            .parse::<isize>()
            .map(|y| {
                let current_year = chrono::Utc::now().year();
                let current_century = (current_year / 100) * 100;
                current_century as isize + y
            })
            .map_err(|_| TenorsError::UnparsableTenorString(s.to_string()))?,
        4 => year_part
            .parse::<isize>()
            .map_err(|_| TenorsError::UnparsableTenorString(s.to_string()))?,
        _ => return Err(TenorsError::UnparsableTenorString(s.to_string())),
    };

    // Create year tenor using epoch offset
    Ok(Tenor::new(TenorType::Year, year - TENOR_EPOCH))
}

fn parse_quarter_format(s: &str) -> Result<Tenor, TenorsError> {
    // Find position of 'Q'
    if let Some(q_pos) = s.find('Q') {
        let before = &s[..q_pos];
        let after = &s[q_pos + 1..];

        // Determine if it's period-first or year-first
        if before.len() == 1 {
            // Period-first format (e.g., "1Q24", "3Q2024")
            let quarter = before
                .parse::<isize>()
                .ok()
                .filter(|&q| q >= 1 && q <= 4)
                .ok_or_else(|| TenorsError::UnparsableTenorString(s.to_string()))?;

            let year = parse_year_part(after, s)?;
            Ok(Tenor::quarter_from_yq0(year, quarter - 1))
        } else {
            // Year-first format (e.g., "24Q1", "2024Q3")
            let year = parse_year_part(before, s)?;

            let quarter = after
                .parse::<isize>()
                .ok()
                .filter(|&q| q >= 1 && q <= 4)
                .ok_or_else(|| TenorsError::UnparsableTenorString(s.to_string()))?;

            Ok(Tenor::quarter_from_yq0(year, quarter - 1))
        }
    } else {
        Err(TenorsError::UnparsableTenorString(s.to_string()))
    }
}

fn parse_halfyear_format(s: &str) -> Result<Tenor, TenorsError> {
    // Find position of 'H'
    if let Some(h_pos) = s.find('H') {
        let before = &s[..h_pos];
        let after = &s[h_pos + 1..];

        // Determine if it's period-first or year-first
        if before.len() == 1 {
            // Period-first format (e.g., "1H24", "2H2024")
            let half = before
                .parse::<isize>()
                .ok()
                .filter(|&h| h >= 1 && h <= 2)
                .ok_or_else(|| TenorsError::UnparsableTenorString(s.to_string()))?;

            let year = parse_year_part(after, s)?;

            // Create half-year tenor
            let ratio = TenorType::Year.granularity() / TenorType::HalfYear.granularity();
            Ok(Tenor {
                tenor_type: TenorType::HalfYear,
                epoch_offset: (year - TENOR_EPOCH) * ratio + (half - 1),
            })
        } else {
            // Year-first format (e.g., "24H1", "2024H2")
            let year = parse_year_part(before, s)?;

            let half = after
                .parse::<isize>()
                .ok()
                .filter(|&h| h >= 1 && h <= 2)
                .ok_or_else(|| TenorsError::UnparsableTenorString(s.to_string()))?;

            // Create half-year tenor
            let ratio = TenorType::Year.granularity() / TenorType::HalfYear.granularity();
            Ok(Tenor {
                tenor_type: TenorType::HalfYear,
                epoch_offset: (year - TENOR_EPOCH) * ratio + (half - 1),
            })
        }
    } else {
        Err(TenorsError::UnparsableTenorString(s.to_string()))
    }
}

fn parse_year_part(year_str: &str, original: &str) -> Result<isize, TenorsError> {
    match year_str.len() {
        2 => {
            // 2-digit year with century offset
            year_str
                .parse::<isize>()
                .map(|y| {
                    let current_year = chrono::Utc::now().year();
                    let current_century = (current_year / 100) * 100;
                    current_century as isize + y
                })
                .map_err(|_| TenorsError::UnparsableTenorString(original.to_string()))
        }
        4 => {
            // Full 4-digit year
            year_str
                .parse::<isize>()
                .map_err(|_| TenorsError::UnparsableTenorString(original.to_string()))
        }
        _ => Err(TenorsError::UnparsableTenorString(original.to_string())),
    }
}

impl Add<isize> for &Tenor {
    type Output = Tenor;
    fn add(self, other: isize) -> Tenor {
        let ret = self
            .epoch_offset
            .checked_add(other)
            .expect("isize over/underflow in tenor!");
        Tenor {
            tenor_type: self.tenor_type,
            epoch_offset: ret,
        }
    }
}

impl Sub<isize> for &Tenor {
    type Output = Tenor;
    fn sub(self, other: isize) -> Tenor {
        let ret = self
            .epoch_offset
            .checked_sub(other)
            .expect("isize over/underflow in tenor!");
        Tenor {
            tenor_type: self.tenor_type,
            epoch_offset: ret,
        }
    }
}

impl AddAssign<isize> for Tenor {
    fn add_assign(&mut self, other: isize) {
        let ret = self
            .epoch_offset
            .checked_add(other)
            .expect("isize over/underflow in tenor!");
        self.epoch_offset = ret;
    }
}

impl SubAssign<isize> for Tenor {
    fn sub_assign(&mut self, other: isize) {
        let ret = self
            .epoch_offset
            .checked_sub(other) // Fixed: was checked_add
            .expect("isize over/underflow in tenor!");
        self.epoch_offset = ret;
    }
}

/// A TenorDuration is a number of lengths of a specific tenor.
/// A duration supports multiplication with a number, e.g. to define 3 quarters.  
/// Approprite durations can be added to a Tenor, e.g. Add 3 quarters to a specific month
// to obtain a shifted month.
#[derive(Debug, PartialEq, Eq)]
pub struct TenorDuration {
    tenor_type: TenorType,
    duration: isize,
}

impl Add<TenorDuration> for &Tenor {
    type Output = Result<Tenor, TenorsError>;
    fn add(self, other: TenorDuration) -> Result<Tenor, TenorsError> {
        if other.tenor_type.granularity() % self.tenor_type.granularity() == 0 {
            Ok(Tenor {
                tenor_type: self.tenor_type,
                epoch_offset: self.epoch_offset
                    + (other.tenor_type.granularity() / self.tenor_type.granularity())
                        * other.duration,
            })
        } else {
            Err(TenorsError::IncompatibleTenors(
                format!("{:?}", self.tenor_type),
                format!("{:?}", other.tenor_type),
            ))
        }
    }
}

impl Sub<TenorDuration> for &Tenor {
    type Output = Result<Tenor, TenorsError>;
    fn sub(self, other: TenorDuration) -> Result<Tenor, TenorsError> {
        if other.tenor_type.granularity() % self.tenor_type.granularity() == 0 {
            Ok(Tenor {
                tenor_type: self.tenor_type,
                epoch_offset: self.epoch_offset
                    - (other.tenor_type.granularity() / self.tenor_type.granularity())
                        * other.duration,
            })
        } else {
            Err(TenorsError::IncompatibleTenors(
                format!("{:?}", self.tenor_type),
                format!("{:?}", other.tenor_type),
            ))
        }
    }
}

impl TenorDuration {
    /// Creates a new tenor duration of one unit of the specified type.
    pub fn new(tenor_type: TenorType) -> TenorDuration {
        TenorDuration {
            tenor_type,
            duration: 1,
        }
    }
}

impl Add<isize> for TenorDuration {
    type Output = Self;
    fn add(self, other: isize) -> Self {
        let ret = self
            .duration
            .checked_add(other)
            .expect("isize over/underflow in tenor!");
        TenorDuration {
            tenor_type: self.tenor_type,
            duration: ret,
        }
    }
}

impl Sub<isize> for TenorDuration {
    type Output = Self;
    fn sub(self, other: isize) -> Self {
        let ret = self
            .duration
            .checked_sub(other)
            .expect("isize over/underflow in tenor!");
        TenorDuration {
            tenor_type: self.tenor_type,
            duration: ret,
        }
    }
}

impl Mul<isize> for TenorDuration {
    type Output = Self;
    fn mul(self, other: isize) -> Self {
        let ret = self
            .duration
            .checked_mul(other)
            .expect("isize over/underflow in tenor!");
        TenorDuration {
            tenor_type: self.tenor_type,
            duration: ret,
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;
    // Check ordering of enum TenorType
    #[test]
    fn check_tenor_type_ord() {
        assert!(TenorType::HalfYear > TenorType::Month);
        assert!(TenorType::HalfMonth < TenorType::Year);
    }
    #[test]
    // add isize to Tenor
    fn check_tenor_add_isize() {
        assert_eq!(
            &Tenor {
                tenor_type: TenorType::HalfMonth,
                epoch_offset: 1
            } + 1,
            Tenor {
                tenor_type: TenorType::HalfMonth,
                epoch_offset: 2
            }
        );
        assert_eq!(
            &Tenor {
                tenor_type: TenorType::Month,
                epoch_offset: 1
            } + 1,
            Tenor {
                tenor_type: TenorType::Month,
                epoch_offset: 2
            }
        );
        assert_eq!(
            &Tenor {
                tenor_type: TenorType::Quarter,
                epoch_offset: 1
            } + 1,
            Tenor {
                tenor_type: TenorType::Quarter,
                epoch_offset: 2
            }
        );
        assert_eq!(
            &Tenor {
                tenor_type: TenorType::HalfYear,
                epoch_offset: 1
            } + 1,
            Tenor {
                tenor_type: TenorType::HalfYear,
                epoch_offset: 2
            }
        );
        assert_eq!(
            &Tenor {
                tenor_type: TenorType::Year,
                epoch_offset: 1
            } + 1,
            Tenor {
                tenor_type: TenorType::Year,
                epoch_offset: 2
            }
        );
    }
    // sub isize from Tenor
    #[test]
    fn check_tenor_sub_isize() {
        assert_eq!(
            &Tenor {
                tenor_type: TenorType::HalfMonth,
                epoch_offset: 1
            } - 1,
            Tenor {
                tenor_type: TenorType::HalfMonth,
                epoch_offset: 0
            }
        );
        assert_eq!(
            &Tenor {
                tenor_type: TenorType::Month,
                epoch_offset: 1
            } - 1,
            Tenor {
                tenor_type: TenorType::Month,
                epoch_offset: 0
            }
        );
        assert_eq!(
            &Tenor {
                tenor_type: TenorType::Quarter,
                epoch_offset: 1
            } - 1,
            Tenor {
                tenor_type: TenorType::Quarter,
                epoch_offset: 0
            }
        );
        assert_eq!(
            &Tenor {
                tenor_type: TenorType::HalfYear,
                epoch_offset: 1
            } - 1,
            Tenor {
                tenor_type: TenorType::HalfYear,
                epoch_offset: 0
            }
        );
        assert_eq!(
            &Tenor {
                tenor_type: TenorType::Year,
                epoch_offset: 1
            } - 1,
            Tenor {
                tenor_type: TenorType::Year,
                epoch_offset: 0
            }
        );
    }
    // test addassign to tenor
    // test subassign to tenor
    // test add isize to duration
    // test sub isize to  duration
    // test mul isize duration
    // test addassign to duration
    // test subassign to duration
    #[test]
    fn check_tenor_add_duration() {
        assert_eq!(
            (&Tenor {
                tenor_type: TenorType::Month,
                epoch_offset: 4,
            } + TenorDuration::new(TenorType::Year) * 4)
                .unwrap(),
            Tenor {
                tenor_type: TenorType::Month,
                epoch_offset: 52
            }
        );
    }
    #[test]
    fn cme_tenor_parser_1() {
        assert_eq!(
            cme_tenor_parser("DEC 32").unwrap(),
            Tenor::month_from_ym0(2032, 11)
        );
    }
    // Test pretty printing
    #[test]
    fn pretty_print_hm() {
        assert_eq!(
            format!("{}", Tenor::new(TenorType::HalfMonth, 1200)),
            "1HF20"
        );
        assert_eq!(
            format!("{}", Tenor::new(TenorType::HalfMonth, 1201)),
            "2HF20"
        );
    }
    // test From trait
    #[test]
    fn test_from1() {
        assert_eq!(
            Tenor::from(&NaiveDate::from_ymd_opt(1970, 1, 4).unwrap()),
            Tenor::new(TenorType::HalfMonth, 0)
        );
        assert_eq!(
            Tenor::from(&NaiveDate::from_ymd_opt(1970, 1, 15).unwrap()),
            Tenor::new(TenorType::HalfMonth, 1)
        );
        assert_eq!(
            Tenor::from(&NaiveDate::from_ymd_opt(2023, 12, 14).unwrap()),
            Tenor::new(TenorType::HalfMonth, 1294)
        );
        assert_eq!(
            Tenor::from(&NaiveDate::from_ymd_opt(2023, 12, 15).unwrap()),
            Tenor::new(TenorType::HalfMonth, 1295)
        );
    }
    // test  futures_code
    #[test]
    fn test_fc1() {
        assert_eq!(
            Tenor::month_from_futures_code("F24").unwrap(),
            Tenor::month_from_ym0(2024, 0)
        );
    }
    #[test]
    fn test_fc2() {
        assert_eq!(
            Tenor::month_from_futures_code("R24"),
            Err(TenorsError::UnparsableTenorString("R24".to_string()))
        );
    }
    #[test]
    fn test_fc3() {
        assert_eq!(
            Tenor::month_from_futures_code("F2t"),
            Err(TenorsError::UnparsableTenorString("F2t".to_string()))
        );
    }
    #[test]
    fn test_fc4() {
        assert_eq!(
            Tenor::month_from_futures_code("R2000"),
            Err(TenorsError::UnparsableTenorString("R2000".to_string()))
        );
    }
    #[test]
    fn test_fc5() {
        assert_eq!(
            Tenor::month_from_futures_code("F2024").unwrap(),
            Tenor::month_from_ym0(2024, 0)
        );
    }

    #[test]
    fn test_from_str_2char_format() {
        // Get current decade for test expectations
        let current_year = chrono::Utc::now().year();
        let current_decade = (current_year / 10) * 10;

        // Test valid 2-char futures codes
        assert_eq!(
            "F7".parse::<Tenor>().unwrap(),
            Tenor::month_from_ym0(current_decade as isize + 7, 0) // F = January, 7 = 7th year of decade
        );
        assert_eq!(
            "Z9".parse::<Tenor>().unwrap(),
            Tenor::month_from_ym0(current_decade as isize + 9, 11) // Z = December, 9 = 9th year of decade
        );
        assert_eq!(
            "M0".parse::<Tenor>().unwrap(),
            Tenor::month_from_ym0(current_decade as isize, 5) // M = June, 0 = 0th year of decade
        );
        assert_eq!(
            "H5".parse::<Tenor>().unwrap(),
            Tenor::month_from_ym0(current_decade as isize + 5, 2) // H = March, 5 = 5th year of decade
        );

        // Test invalid 2-char codes
        assert_eq!(
            "A7".parse::<Tenor>(),
            Err(TenorsError::UnparsableTenorString("A7".to_string())) // A is not a valid futures code
        );
        assert_eq!(
            "FA".parse::<Tenor>(),
            Err(TenorsError::UnparsableTenorString("FA".to_string())) // Second char must be digit
        );

        // Test that other lengths don't match this format
        assert_eq!(
            "F".parse::<Tenor>(),
            Err(TenorsError::UnparsableTenorString("F".to_string()))
        );
    }

    #[test]
    fn test_from_str_3char_format() {
        // Get current century for test expectations
        let current_year = chrono::Utc::now().year();
        let current_century = (current_year / 100) * 100;

        // Test valid 3-char futures codes
        assert_eq!(
            "F24".parse::<Tenor>().unwrap(),
            Tenor::month_from_ym0(current_century as isize + 24, 0) // F = January, 24 = 2024
        );
        assert_eq!(
            "Z99".parse::<Tenor>().unwrap(),
            Tenor::month_from_ym0(current_century as isize + 99, 11) // Z = December, 99 = 2099
        );
        assert_eq!(
            "M00".parse::<Tenor>().unwrap(),
            Tenor::month_from_ym0(current_century as isize, 5) // M = June, 00 = 2000
        );
        assert_eq!(
            "H45".parse::<Tenor>().unwrap(),
            Tenor::month_from_ym0(current_century as isize + 45, 2) // H = March, 45 = 2045
        );
        assert_eq!(
            "Q03".parse::<Tenor>().unwrap(),
            Tenor::month_from_ym0(current_century as isize + 3, 7) // Q = August, 03 = 2003
        );

        // Test invalid 3-char codes
        assert_eq!(
            "A24".parse::<Tenor>(),
            Err(TenorsError::UnparsableTenorString("A24".to_string())) // A is not a valid futures code
        );
        assert_eq!(
            "F2A".parse::<Tenor>(),
            Err(TenorsError::UnparsableTenorString("F2A".to_string())) // Last two chars must be digits
        );
        assert_eq!(
            "FX4".parse::<Tenor>(),
            Err(TenorsError::UnparsableTenorString("FX4".to_string())) // Second char must be digit
        );
    }

    #[test]
    fn test_from_str_5char_format() {
        // Test valid 5-char futures codes with full 4-digit year
        assert_eq!(
            "F2024".parse::<Tenor>().unwrap(),
            Tenor::month_from_ym0(2024, 0) // F = January, 2024 = full year
        );
        assert_eq!(
            "Z2099".parse::<Tenor>().unwrap(),
            Tenor::month_from_ym0(2099, 11) // Z = December, 2099 = full year
        );
        assert_eq!(
            "M1970".parse::<Tenor>().unwrap(),
            Tenor::month_from_ym0(1970, 5) // M = June, 1970 = full year
        );
        assert_eq!(
            "H2045".parse::<Tenor>().unwrap(),
            Tenor::month_from_ym0(2045, 2) // H = March, 2045 = full year
        );
        assert_eq!(
            "Q2003".parse::<Tenor>().unwrap(),
            Tenor::month_from_ym0(2003, 7) // Q = August, 2003 = full year
        );
        assert_eq!(
            "F3000".parse::<Tenor>().unwrap(),
            Tenor::month_from_ym0(3000, 0) // Far future year
        );

        // Test invalid 5-char codes
        assert_eq!(
            "A2024".parse::<Tenor>(),
            Err(TenorsError::UnparsableTenorString("A2024".to_string())) // A is not a valid futures code
        );
        assert_eq!(
            "F202A".parse::<Tenor>(),
            Err(TenorsError::UnparsableTenorString("F202A".to_string())) // Year must be all digits
        );
        assert_eq!(
            "FX024".parse::<Tenor>(),
            Err(TenorsError::UnparsableTenorString("FX024".to_string())) // Second char must be digit
        );
    }

    #[test]
    fn test_from_str_no_overlap() {
        // Verify that all formats don't overlap
        // They are distinguished by length, so no overlap is possible

        // 2-char format examples (decade offset)
        assert!("F7".parse::<Tenor>().is_ok());
        assert!("M0".parse::<Tenor>().is_ok());

        // 3-char format examples (century offset)
        assert!("F24".parse::<Tenor>().is_ok());
        assert!("M00".parse::<Tenor>().is_ok());

        // 5-char format examples (full year)
        assert!("F2024".parse::<Tenor>().is_ok());
        assert!("M1970".parse::<Tenor>().is_ok());

        // Invalid lengths (1, 4, 6+ chars)
        assert!("F".parse::<Tenor>().is_err());
        assert!("F202".parse::<Tenor>().is_err()); // 4 chars - not supported
        assert!("F20245".parse::<Tenor>().is_err()); // 6 chars - not supported
    }

    #[test]
    fn test_from_str_year_format() {
        let current_year = chrono::Utc::now().year();
        let current_century = (current_year / 100) * 100;

        // Test basic C prefix with 2-digit year
        assert_eq!(
            "C24".parse::<Tenor>().unwrap(),
            Tenor::new(
                TenorType::Year,
                (current_century as isize + 24) - TENOR_EPOCH
            )
        );
        assert_eq!(
            "C00".parse::<Tenor>().unwrap(),
            Tenor::new(TenorType::Year, current_century as isize - TENOR_EPOCH)
        );

        // Test C prefix with 4-digit year
        assert_eq!(
            "C2024".parse::<Tenor>().unwrap(),
            Tenor::new(TenorType::Year, 2024 - TENOR_EPOCH)
        );
        assert_eq!(
            "C1970".parse::<Tenor>().unwrap(),
            Tenor::new(TenorType::Year, 0) // 1970 is the epoch
        );

        // Test with "al" (case insensitive)
        assert_eq!(
            "Cal24".parse::<Tenor>().unwrap(),
            Tenor::new(
                TenorType::Year,
                (current_century as isize + 24) - TENOR_EPOCH
            )
        );
        assert_eq!(
            "CAL24".parse::<Tenor>().unwrap(),
            Tenor::new(
                TenorType::Year,
                (current_century as isize + 24) - TENOR_EPOCH
            )
        );
        assert_eq!(
            "CaL24".parse::<Tenor>().unwrap(),
            Tenor::new(
                TenorType::Year,
                (current_century as isize + 24) - TENOR_EPOCH
            )
        );
        assert_eq!(
            "Cal2024".parse::<Tenor>().unwrap(),
            Tenor::new(TenorType::Year, 2024 - TENOR_EPOCH)
        );

        // Test invalid formats
        assert!("C".parse::<Tenor>().is_err());
        assert!("C2".parse::<Tenor>().is_err()); // Only 1 digit
        assert!("C202".parse::<Tenor>().is_err()); // 3 digits
        assert!("Cal".parse::<Tenor>().is_err()); // No year
        assert!("Cal2".parse::<Tenor>().is_err()); // Only 1 digit after "al"
        assert!("D24".parse::<Tenor>().is_err()); // Wrong prefix
    }

    #[test]
    fn test_from_str_quarter_format() {
        let current_year = chrono::Utc::now().year();
        let current_century = (current_year / 100) * 100;

        // Test period-first format with 2-digit year
        assert_eq!(
            "1Q24".parse::<Tenor>().unwrap(),
            Tenor::quarter_from_yq0(current_century as isize + 24, 0)
        );
        assert_eq!(
            "2Q25".parse::<Tenor>().unwrap(),
            Tenor::quarter_from_yq0(current_century as isize + 25, 1)
        );
        assert_eq!(
            "3Q26".parse::<Tenor>().unwrap(),
            Tenor::quarter_from_yq0(current_century as isize + 26, 2)
        );
        assert_eq!(
            "4Q27".parse::<Tenor>().unwrap(),
            Tenor::quarter_from_yq0(current_century as isize + 27, 3)
        );

        // Test period-first format with 4-digit year
        assert_eq!(
            "1Q2024".parse::<Tenor>().unwrap(),
            Tenor::quarter_from_yq0(2024, 0)
        );
        assert_eq!(
            "4Q2025".parse::<Tenor>().unwrap(),
            Tenor::quarter_from_yq0(2025, 3)
        );

        // Test year-first format with 2-digit year
        assert_eq!(
            "24Q1".parse::<Tenor>().unwrap(),
            Tenor::quarter_from_yq0(current_century as isize + 24, 0)
        );
        assert_eq!(
            "25Q4".parse::<Tenor>().unwrap(),
            Tenor::quarter_from_yq0(current_century as isize + 25, 3)
        );

        // Test year-first format with 4-digit year
        assert_eq!(
            "2024Q1".parse::<Tenor>().unwrap(),
            Tenor::quarter_from_yq0(2024, 0)
        );
        assert_eq!(
            "2025Q4".parse::<Tenor>().unwrap(),
            Tenor::quarter_from_yq0(2025, 3)
        );

        // Test invalid quarter numbers
        assert!("0Q24".parse::<Tenor>().is_err());
        assert!("5Q24".parse::<Tenor>().is_err());
        assert!("24Q0".parse::<Tenor>().is_err());
        assert!("24Q5".parse::<Tenor>().is_err());

        // Test that Q without proper format is parsed as month
        assert_eq!(
            "Q24".parse::<Tenor>().unwrap().get_tenor_type(),
            TenorType::Month
        ); // August 2024
        assert_eq!(
            "Q2024".parse::<Tenor>().unwrap().get_tenor_type(),
            TenorType::Month
        ); // August 2024
    }

    #[test]
    fn test_from_str_halfyear_format() {
        let current_year = chrono::Utc::now().year();
        let current_century = (current_year / 100) * 100;

        // Test period-first format with 2-digit year
        assert_eq!(
            "1H24".parse::<Tenor>().unwrap(),
            Tenor {
                tenor_type: TenorType::HalfYear,
                epoch_offset: ((current_century as isize + 24) - TENOR_EPOCH) * 2 + 0
            }
        );
        assert_eq!(
            "2H25".parse::<Tenor>().unwrap(),
            Tenor {
                tenor_type: TenorType::HalfYear,
                epoch_offset: ((current_century as isize + 25) - TENOR_EPOCH) * 2 + 1
            }
        );

        // Test period-first format with 4-digit year
        assert_eq!(
            "1H2024".parse::<Tenor>().unwrap(),
            Tenor {
                tenor_type: TenorType::HalfYear,
                epoch_offset: (2024 - TENOR_EPOCH) * 2 + 0
            }
        );
        assert_eq!(
            "2H2025".parse::<Tenor>().unwrap(),
            Tenor {
                tenor_type: TenorType::HalfYear,
                epoch_offset: (2025 - TENOR_EPOCH) * 2 + 1
            }
        );

        // Test year-first format with 2-digit year
        assert_eq!(
            "24H1".parse::<Tenor>().unwrap(),
            Tenor {
                tenor_type: TenorType::HalfYear,
                epoch_offset: ((current_century as isize + 24) - TENOR_EPOCH) * 2 + 0
            }
        );
        assert_eq!(
            "25H2".parse::<Tenor>().unwrap(),
            Tenor {
                tenor_type: TenorType::HalfYear,
                epoch_offset: ((current_century as isize + 25) - TENOR_EPOCH) * 2 + 1
            }
        );

        // Test year-first format with 4-digit year
        assert_eq!(
            "2024H1".parse::<Tenor>().unwrap(),
            Tenor {
                tenor_type: TenorType::HalfYear,
                epoch_offset: (2024 - TENOR_EPOCH) * 2 + 0
            }
        );
        assert_eq!(
            "2025H2".parse::<Tenor>().unwrap(),
            Tenor {
                tenor_type: TenorType::HalfYear,
                epoch_offset: (2025 - TENOR_EPOCH) * 2 + 1
            }
        );

        // Test invalid half numbers
        assert!("0H24".parse::<Tenor>().is_err());
        assert!("3H24".parse::<Tenor>().is_err());
        assert!("24H0".parse::<Tenor>().is_err());
        assert!("24H3".parse::<Tenor>().is_err());

        // Test that H without proper format is parsed as month
        assert_eq!(
            "H24".parse::<Tenor>().unwrap().get_tenor_type(),
            TenorType::Month
        ); // March 2024
        assert_eq!(
            "H2024".parse::<Tenor>().unwrap().get_tenor_type(),
            TenorType::Month
        ); // March 2024
    }

    #[test]
    fn test_from_str_all_formats_no_conflict() {
        // Verify month futures codes are parsed correctly (not confused with Q/H)
        assert_eq!(
            "Q24".parse::<Tenor>().unwrap().get_tenor_type(),
            TenorType::Month // August 2024
        );
        assert_eq!(
            "H24".parse::<Tenor>().unwrap().get_tenor_type(),
            TenorType::Month // March 2024
        );

        // Verify quarters are parsed correctly
        assert_eq!(
            "1Q24".parse::<Tenor>().unwrap().get_tenor_type(),
            TenorType::Quarter
        );
        assert_eq!(
            "24Q1".parse::<Tenor>().unwrap().get_tenor_type(),
            TenorType::Quarter
        );

        // Verify half-years are parsed correctly
        assert_eq!(
            "1H24".parse::<Tenor>().unwrap().get_tenor_type(),
            TenorType::HalfYear
        );
        assert_eq!(
            "24H1".parse::<Tenor>().unwrap().get_tenor_type(),
            TenorType::HalfYear
        );

        // Verify years are parsed correctly
        assert_eq!(
            "C24".parse::<Tenor>().unwrap().get_tenor_type(),
            TenorType::Year
        );
        assert_eq!(
            "Cal24".parse::<Tenor>().unwrap().get_tenor_type(),
            TenorType::Year
        );
    }

    #[test]
    fn test_checked_arithmetic() {
        let tenor = Tenor::new(TenorType::Month, 100);

        // Test checked_add
        assert_eq!(
            tenor.checked_add(10),
            Some(Tenor::new(TenorType::Month, 110))
        );
        assert_eq!(
            tenor.checked_add(isize::MAX),
            None // Overflow
        );

        // Test checked_sub
        assert_eq!(
            tenor.checked_sub(10),
            Some(Tenor::new(TenorType::Month, 90))
        );

        // Test edge case for underflow
        let min_tenor = Tenor::new(TenorType::Month, isize::MIN);
        assert_eq!(
            min_tenor.checked_sub(1),
            None // Underflow
        );

        // Test saturating arithmetic
        assert_eq!(
            tenor.saturating_add(isize::MAX),
            Tenor::new(TenorType::Month, isize::MAX)
        );
        // 100.saturating_sub(MAX) = 100 - MAX with saturation = -9223372036854775707
        assert_eq!(
            tenor.saturating_sub(isize::MAX),
            Tenor::new(TenorType::Month, 100_isize.saturating_sub(isize::MAX))
        );
    }

    #[test]
    fn test_subassign_bug_fixed() {
        let mut tenor = Tenor::new(TenorType::Month, 100);
        tenor -= 10;
        assert_eq!(tenor, Tenor::new(TenorType::Month, 90));
    }

    #[test]
    fn test_cmp_by_granularity() {
        // Create tenors of different types
        let hm1 = Tenor::new(TenorType::HalfMonth, 1296); // 1HJan24
        let hm2 = Tenor::new(TenorType::HalfMonth, 1297); // 2HJan24
        let m1 = Tenor::month_from_ym0(2024, 0); // Jan24
        let m2 = Tenor::month_from_ym0(2024, 1); // Feb24
        let q1 = Tenor::quarter_from_yq0(2024, 0); // 1Q24
        let q2 = Tenor::quarter_from_yq0(2024, 1); // 2Q24
        let h1 = Tenor::new(TenorType::HalfYear, 108); // 1H24
        let h2 = Tenor::new(TenorType::HalfYear, 109); // 2H24
        let y1 = Tenor::new(TenorType::Year, 54); // 2024
        let y2 = Tenor::new(TenorType::Year, 55); // 2025

        // Test ordering within same type
        assert_eq!(hm1.cmp_by_granularity(&hm2), Ordering::Less);
        assert_eq!(m1.cmp_by_granularity(&m2), Ordering::Less);
        assert_eq!(q1.cmp_by_granularity(&q2), Ordering::Less);
        assert_eq!(h1.cmp_by_granularity(&h2), Ordering::Less);
        assert_eq!(y1.cmp_by_granularity(&y2), Ordering::Less);

        // Test ordering between types (finer granularity comes first)
        assert_eq!(hm1.cmp_by_granularity(&m1), Ordering::Less);
        assert_eq!(m1.cmp_by_granularity(&q1), Ordering::Less);
        assert_eq!(q1.cmp_by_granularity(&h1), Ordering::Less);
        assert_eq!(h1.cmp_by_granularity(&y1), Ordering::Less);

        // Test sorting a mixed vector
        let mut tenors = vec![
            y1.clone(),
            q2.clone(),
            hm1.clone(),
            m2.clone(),
            h1.clone(),
            q1.clone(),
            hm2.clone(),
            m1.clone(),
            h2.clone(),
            y2.clone(),
        ];
        tenors.sort_by(Tenor::cmp_by_granularity);

        assert_eq!(tenors[0], hm1);
        assert_eq!(tenors[1], hm2);
        assert_eq!(tenors[2], m1);
        assert_eq!(tenors[3], m2);
        assert_eq!(tenors[4], q1);
        assert_eq!(tenors[5], q2);
        assert_eq!(tenors[6], h1);
        assert_eq!(tenors[7], h2);
        assert_eq!(tenors[8], y1);
        assert_eq!(tenors[9], y2);
    }

    #[test]
    fn test_quarter0() {
        // Test quarter tenors return correct quarter0
        assert_eq!(Tenor::quarter_from_yq0(2024, 0).quarter0(), Some(0)); // Q1
        assert_eq!(Tenor::quarter_from_yq0(2024, 1).quarter0(), Some(1)); // Q2
        assert_eq!(Tenor::quarter_from_yq0(2024, 2).quarter0(), Some(2)); // Q3
        assert_eq!(Tenor::quarter_from_yq0(2024, 3).quarter0(), Some(3)); // Q4

        // Test month tenors return correct quarter0
        assert_eq!(Tenor::month_from_ym0(2024, 0).quarter0(), Some(0)); // Jan -> Q1
        assert_eq!(Tenor::month_from_ym0(2024, 1).quarter0(), Some(0)); // Feb -> Q1
        assert_eq!(Tenor::month_from_ym0(2024, 2).quarter0(), Some(0)); // Mar -> Q1
        assert_eq!(Tenor::month_from_ym0(2024, 3).quarter0(), Some(1)); // Apr -> Q2
        assert_eq!(Tenor::month_from_ym0(2024, 4).quarter0(), Some(1)); // May -> Q2
        assert_eq!(Tenor::month_from_ym0(2024, 5).quarter0(), Some(1)); // Jun -> Q2
        assert_eq!(Tenor::month_from_ym0(2024, 6).quarter0(), Some(2)); // Jul -> Q3
        assert_eq!(Tenor::month_from_ym0(2024, 7).quarter0(), Some(2)); // Aug -> Q3
        assert_eq!(Tenor::month_from_ym0(2024, 8).quarter0(), Some(2)); // Sep -> Q3
        assert_eq!(Tenor::month_from_ym0(2024, 9).quarter0(), Some(3)); // Oct -> Q4
        assert_eq!(Tenor::month_from_ym0(2024, 10).quarter0(), Some(3)); // Nov -> Q4
        assert_eq!(Tenor::month_from_ym0(2024, 11).quarter0(), Some(3)); // Dec -> Q4

        // Test half-month tenors return correct quarter0
        assert_eq!(Tenor::new(TenorType::HalfMonth, 1296).quarter0(), Some(0)); // 1HJan24 -> Q1
        assert_eq!(Tenor::new(TenorType::HalfMonth, 1302).quarter0(), Some(1)); // 1HApr24 -> Q2
        assert_eq!(Tenor::new(TenorType::HalfMonth, 1308).quarter0(), Some(2)); // 1HJul24 -> Q3
        assert_eq!(Tenor::new(TenorType::HalfMonth, 1314).quarter0(), Some(3)); // 1HOct24 -> Q4

        // Test coarser tenor types return None
        assert_eq!(Tenor::new(TenorType::HalfYear, 108).quarter0(), None); // H1 2024
        assert_eq!(Tenor::new(TenorType::Year, 54).quarter0(), None); // 2024
    }
}
