//! Interpolation for forward curves quoted at mixed granularities.
//!
//! A coarser tenor (a quarter) *covers* finer ones (its three months). When a
//! curve carries the coarser value and some of the finer ones, the
//! arithmetic-average constraint
//!
//! ```text
//! value(cover) = average(value(partition elements))
//! ```
//!
//! determines what the missing finer values must sum to; they are filled
//! flat. Covers are processed finest-first, so the tightest constraint fills
//! first and a coarser cover only distributes over what is still missing;
//! covers that contradict each other are reported, never silently averaged.
//!
//! # Example
//!
//! ```
//! use tenors::TenorType;
//! use tenors::interpolation::{TenorPoint, interpolate_from_covers};
//!
//! let mut curve = vec![
//!     TenorPoint::new("F25".parse().unwrap(), 100.0),
//!     TenorPoint::new("G25".parse().unwrap(), 105.0),
//!     TenorPoint::new("1Q25".parse().unwrap(), 103.33),
//! ];
//! let added = interpolate_from_covers(&mut curve, TenorType::Month, 1e-9, TenorPoint::new).unwrap();
//! assert_eq!(added.len(), 1); // H25
//! ```

use std::collections::HashMap;

use thiserror::Error;

use crate::{Tenor, TenorType};

/// Errors from curve interpolation and validation.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum InterpolationError {
    /// A cover's value is not the average of its partition elements.
    #[error(
        "inconsistent curve: cover {cover} has value {cover_value}, but its partition averages {partition_avg}"
    )]
    InconsistentCurve {
        /// The cover tenor.
        cover: Tenor,
        /// The cover's value.
        cover_value: f64,
        /// The average of its partition elements.
        partition_avg: f64,
    },
}

/// Something that pairs a tenor with a value.
pub trait TenorValue {
    /// The tenor.
    fn tenor(&self) -> Tenor;
    /// The value.
    fn value(&self) -> f64;
    /// Replace the value.
    fn set_value(&mut self, value: f64);
}

/// The plain tenor–value pair.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TenorPoint {
    /// The tenor.
    pub tenor: Tenor,
    /// The value.
    pub value: f64,
}

impl TenorPoint {
    /// A new point.
    pub fn new(tenor: Tenor, value: f64) -> Self {
        Self { tenor, value }
    }
}

impl TenorValue for TenorPoint {
    fn tenor(&self) -> Tenor {
        self.tenor
    }
    fn value(&self) -> f64 {
        self.value
    }
    fn set_value(&mut self, value: f64) {
        self.value = value;
    }
}

/// Fill in the `target_type` tenors that the curve's coarser covers imply.
///
/// Every tenor coarser than `target_type` is a cover; covers are processed
/// finest-first. For each, the missing partition elements get the same value,
/// chosen so that the partition averages to the cover's value; if none of the
/// elements exist yet, they all get the cover's value. New points are created
/// with `create_point` and appended to `curve`.
///
/// After filling, every cover whose partition is complete is checked; a cover
/// whose value differs from its partition's average by more than `tolerance`
/// makes the call fail with [`InterpolationError::InconsistentCurve`] —
/// contradictory covers are an error, not a silently averaged curve. The
/// curve is left with the points added so far.
///
/// Returns the tenors that were added.
pub fn interpolate_from_covers<T: TenorValue>(
    curve: &mut Vec<T>,
    target_type: TenorType,
    tolerance: f64,
    create_point: impl Fn(Tenor, f64) -> T,
) -> Result<Vec<Tenor>, InterpolationError> {
    let mut index: HashMap<Tenor, usize> = curve
        .iter()
        .enumerate()
        .map(|(i, p)| (p.tenor(), i))
        .collect();

    let mut covers: Vec<(Tenor, f64)> = curve
        .iter()
        .filter(|p| p.tenor().tenor_type() > target_type)
        .map(|p| (p.tenor(), p.value()))
        .collect();
    covers.sort_by(|a, b| a.0.cmp_by_granularity(&b.0));

    let mut added = Vec::new();
    for (cover, cover_value) in covers {
        let Some(partition) = cover.partition(target_type) else {
            continue;
        };
        let (mut present_sum, mut present_count) = (0.0, 0usize);
        let mut missing = Vec::new();
        for part in &partition {
            match index.get(part) {
                Some(&i) => {
                    present_sum += curve[i].value();
                    present_count += 1;
                }
                None => missing.push(*part),
            }
        }
        if missing.is_empty() {
            continue;
        }
        let fill = if present_count == 0 {
            cover_value
        } else {
            (partition.len() as f64 * cover_value - present_sum) / missing.len() as f64
        };
        for tenor in missing {
            index.insert(tenor, curve.len());
            curve.push(create_point(tenor, fill));
            added.push(tenor);
        }
    }

    validate_curve(curve, target_type, tolerance)?;
    Ok(added)
}

/// Check every cover whose partition at `target_type` is complete: its value
/// must equal the partition's average within `tolerance`. Covers with missing
/// partition elements are skipped.
pub fn validate_curve<T: TenorValue>(
    curve: &[T],
    target_type: TenorType,
    tolerance: f64,
) -> Result<(), InterpolationError> {
    let values: HashMap<Tenor, f64> = curve.iter().map(|p| (p.tenor(), p.value())).collect();
    for point in curve {
        let cover = point.tenor();
        if cover.tenor_type() <= target_type {
            continue;
        }
        let Some(partition) = cover.partition(target_type) else {
            continue;
        };
        let found: Vec<f64> = partition
            .iter()
            .filter_map(|t| values.get(t).copied())
            .collect();
        if found.len() != partition.len() {
            continue;
        }
        let partition_avg = found.iter().sum::<f64>() / found.len() as f64;
        if (partition_avg - point.value()).abs() > tolerance {
            return Err(InterpolationError::InconsistentCurve {
                cover,
                cover_value: point.value(),
                partition_avg,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> Tenor {
        s.parse().unwrap()
    }
    fn value(curve: &[TenorPoint], s: &str) -> f64 {
        curve
            .iter()
            .find(|p| p.tenor == t(s))
            .unwrap_or_else(|| panic!("{s} missing"))
            .value
    }
    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn single_missing_month() {
        let mut curve = vec![
            TenorPoint::new(t("F25"), 100.0),
            TenorPoint::new(t("G25"), 105.0),
            TenorPoint::new(t("1Q25"), 103.33),
        ];
        let added =
            interpolate_from_covers(&mut curve, TenorType::Month, 1e-9, TenorPoint::new).unwrap();
        assert_eq!(added, [t("H25")]);
        assert!(near(value(&curve, "H25"), 3.0 * 103.33 - 205.0));
    }

    #[test]
    fn multiple_missing_months_share_the_residual() {
        let mut curve = vec![
            TenorPoint::new(t("F25"), 90.0),
            TenorPoint::new(t("1Q25"), 100.0),
        ];
        let added =
            interpolate_from_covers(&mut curve, TenorType::Month, 1e-9, TenorPoint::new).unwrap();
        assert_eq!(added.len(), 2);
        assert!(near(value(&curve, "G25"), 105.0) && near(value(&curve, "H25"), 105.0));
    }

    #[test]
    fn all_missing_is_flat_at_the_cover() {
        let mut curve = vec![TenorPoint::new(t("1Q25"), 100.0)];
        let added =
            interpolate_from_covers(&mut curve, TenorType::Month, 1e-9, TenorPoint::new).unwrap();
        assert_eq!(added.len(), 3);
        assert!(
            ["F25", "G25", "H25"]
                .iter()
                .all(|m| near(value(&curve, m), 100.0))
        );
    }

    #[test]
    fn nothing_to_do() {
        let mut curve = vec![
            TenorPoint::new(t("F25"), 100.0),
            TenorPoint::new(t("G25"), 105.0),
            TenorPoint::new(t("H25"), 110.0),
            TenorPoint::new(t("1Q25"), 105.0),
        ];
        assert!(
            interpolate_from_covers(&mut curve, TenorType::Month, 1e-9, TenorPoint::new)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn year_to_quarters_and_half_year_to_months() {
        let mut curve = vec![
            TenorPoint::new(t("1Q25"), 90.0),
            TenorPoint::new(t("2Q25"), 95.0),
            TenorPoint::new(t("Cal25"), 100.0),
        ];
        let added =
            interpolate_from_covers(&mut curve, TenorType::Quarter, 1e-9, TenorPoint::new).unwrap();
        assert_eq!(added.len(), 2);
        assert!(near(value(&curve, "3Q25"), 107.5) && near(value(&curve, "4Q25"), 107.5));

        let mut curve = vec![
            TenorPoint::new(t("F25"), 90.0),
            TenorPoint::new(t("G25"), 95.0),
            TenorPoint::new(t("H25"), 100.0),
            TenorPoint::new(t("J25"), 105.0),
            TenorPoint::new(t("K25"), 110.0),
            TenorPoint::new(t("1H25"), 100.0),
        ];
        let added =
            interpolate_from_covers(&mut curve, TenorType::Month, 1e-9, TenorPoint::new).unwrap();
        assert_eq!(added, [t("M25")]);
        assert!(near(value(&curve, "M25"), 100.0));
    }

    /// The case from the 2026-09 review: the same six points gave a different,
    /// self-contradicting curve when the yearly cover came first.
    #[test]
    fn nested_covers_are_order_independent() {
        let points = [
            ("F25", 100.0),
            ("G25", 101.0),
            ("H25", 102.0),
            ("2Q25", 104.0),
            ("2H25", 110.0),
            ("Cal25", 106.25),
        ];
        let mut results = Vec::new();
        for order in [[0, 1, 2, 3, 4, 5], [5, 4, 3, 2, 1, 0], [3, 5, 0, 4, 2, 1]] {
            let mut curve: Vec<TenorPoint> = order
                .iter()
                .map(|&i| TenorPoint::new(t(points[i].0), points[i].1))
                .collect();
            interpolate_from_covers(&mut curve, TenorType::Month, 1e-9, TenorPoint::new).unwrap();
            curve.sort_by(|a, b| a.tenor.cmp_by_granularity(&b.tenor));
            assert!(validate_curve(&curve, TenorType::Month, 1e-9).is_ok());
            results.push(
                curve
                    .iter()
                    .map(|p| (p.tenor, (p.value * 1e6).round()))
                    .collect::<Vec<_>>(),
            );
        }
        assert_eq!(results[0], results[1]);
        assert_eq!(results[0], results[2]);
        let curve: Vec<TenorPoint> = results[0]
            .iter()
            .map(|&(tn, v)| TenorPoint::new(tn, v / 1e6))
            .collect();
        assert!(
            ["J25", "K25", "M25"]
                .iter()
                .all(|m| near(value(&curve, m), 104.0))
        );
        assert!(
            ["N25", "Q25", "U25", "V25", "X25", "Z25"]
                .iter()
                .all(|m| near(value(&curve, m), 110.0))
        );
    }

    #[test]
    fn contradictory_covers_are_an_error() {
        let mut curve = vec![
            TenorPoint::new(t("1Q25"), 100.0),
            TenorPoint::new(t("2Q25"), 100.0),
            TenorPoint::new(t("3Q25"), 100.0),
            TenorPoint::new(t("4Q25"), 100.0),
            TenorPoint::new(t("Cal25"), 120.0),
        ];
        let err = interpolate_from_covers(&mut curve, TenorType::Month, 1e-9, TenorPoint::new)
            .unwrap_err();
        assert!(
            matches!(err, InterpolationError::InconsistentCurve { cover, cover_value, partition_avg } if cover == t("Cal25") && cover_value == 120.0 && near(partition_avg, 100.0))
        );
        // the quarters were filled before the contradiction was detected
        assert_eq!(curve.len(), 5 + 12);
    }

    #[test]
    fn validate_consistent_inconsistent_and_incomplete() {
        let consistent = vec![
            TenorPoint::new(t("F25"), 100.0),
            TenorPoint::new(t("G25"), 105.0),
            TenorPoint::new(t("H25"), 110.0),
            TenorPoint::new(t("1Q25"), 105.0),
        ];
        assert!(validate_curve(&consistent, TenorType::Month, 0.01).is_ok());
        let mut inconsistent = consistent.clone();
        inconsistent[3].value = 120.0;
        assert!(matches!(
            validate_curve(&inconsistent, TenorType::Month, 0.01),
            Err(InterpolationError::InconsistentCurve { .. })
        ));
        let incomplete = vec![
            TenorPoint::new(t("F25"), 100.0),
            TenorPoint::new(t("1Q25"), 120.0),
        ];
        assert!(validate_curve(&incomplete, TenorType::Month, 0.01).is_ok());
    }

    #[test]
    fn custom_tenor_value_type() {
        #[derive(Clone)]
        struct Row {
            tenor: Tenor,
            price: f64,
            label: &'static str,
        }
        impl TenorValue for Row {
            fn tenor(&self) -> Tenor {
                self.tenor
            }
            fn value(&self) -> f64 {
                self.price
            }
            fn set_value(&mut self, value: f64) {
                self.price = value;
            }
        }
        let mut rows = vec![
            Row {
                tenor: t("F25"),
                price: 90.0,
                label: "quoted",
            },
            Row {
                tenor: t("1Q25"),
                price: 100.0,
                label: "quoted",
            },
        ];
        let added =
            interpolate_from_covers(&mut rows, TenorType::Month, 1e-9, |tenor, price| Row {
                tenor,
                price,
                label: "derived",
            })
            .unwrap();
        assert_eq!(added.len(), 2);
        assert_eq!(rows.iter().filter(|r| r.label == "derived").count(), 2);
        rows[0].set_value(1.0);
        assert_eq!(rows[0].value(), 1.0);
    }
}
