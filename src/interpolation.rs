//! Interpolation for forward curves with mixed tenor granularities.
//!
//! This module provides functionality to interpolate missing tenor values
//! in forward curves using cover constraints. When a coarser tenor (e.g., a quarter)
//! covers finer tenors (e.g., months), the arithmetic average constraint can be
//! used to infer missing values.
//!
//! # Example
//!
//! ```
//! use tenors::{Tenor, TenorType};
//! use tenors::interpolation::{TenorPoint, interpolate_from_covers};
//!
//! let mut curve = vec![
//!     TenorPoint::new("F25".parse().unwrap(), 100.0),
//!     TenorPoint::new("G25".parse().unwrap(), 105.0),
//!     TenorPoint::new("1Q25".parse().unwrap(), 103.33),
//! ];
//!
//! // Interpolate missing months using quarter constraints
//! let interpolated = interpolate_from_covers(
//!     &mut curve,
//!     TenorType::Month,
//!     TenorPoint::new,
//! ).unwrap();
//!
//! // H25 (March 2025) was interpolated
//! assert_eq!(interpolated.len(), 1);
//! ```

use std::collections::HashMap;

use thiserror::Error;

use crate::{Tenor, TenorType};

/// Errors that can occur during curve interpolation.
#[derive(Error, Debug)]
pub enum InterpolationError {
    /// No cover constraint is available to interpolate the given tenor.
    #[error("no cover constraint available for tenor {0}")]
    NoCoverConstraint(Tenor),

    /// The curve is inconsistent: the cover value doesn't match the partition average.
    #[error("inconsistent curve: cover {cover} has value {cover_value}, but partition average is {partition_avg}")]
    InconsistentCurve {
        /// The cover tenor.
        cover: Tenor,
        /// The value assigned to the cover.
        cover_value: f64,
        /// The computed average of the partition elements.
        partition_avg: f64,
    },

    /// All partition elements were missing (now handled with constant interpolation).
    /// This variant is kept for backwards compatibility but is no longer returned.
    #[error("all partition elements missing for {0} (using constant interpolation)")]
    #[allow(dead_code)]
    AllPartitionElementsMissing(Tenor),
}

/// Trait for objects that associate a tenor with a numeric value.
///
/// Implement this trait for your own types to use them with the interpolation functions.
pub trait TenorValue {
    /// Returns a reference to the tenor.
    fn tenor(&self) -> &Tenor;

    /// Returns the numeric value associated with this tenor.
    fn value(&self) -> f64;

    /// Sets the numeric value.
    fn set_value(&mut self, value: f64);
}

/// A simple tenor-value pair.
///
/// This is a convenient default implementation of [`TenorValue`] for basic use cases.
#[derive(Debug, Clone, PartialEq)]
pub struct TenorPoint {
    /// The tenor.
    pub tenor: Tenor,
    /// The value.
    pub value: f64,
}

impl TenorPoint {
    /// Creates a new tenor point.
    pub fn new(tenor: Tenor, value: f64) -> Self {
        Self { tenor, value }
    }
}

impl TenorValue for TenorPoint {
    fn tenor(&self) -> &Tenor {
        &self.tenor
    }

    fn value(&self) -> f64 {
        self.value
    }

    fn set_value(&mut self, value: f64) {
        self.value = value;
    }
}

/// Interpolates missing finer-granularity tenors using cover constraints.
///
/// For each coarser tenor in the curve, this function checks if its partition elements
/// (at `target_type` granularity) exist. Missing elements are filled in using the
/// arithmetic average constraint:
///
/// ```text
/// value(cover) = average(value(partition_elements))
/// ```
///
/// # Arguments
///
/// * `curve` - The curve to interpolate, modified in place with new points added.
/// * `target_type` - The granularity to interpolate to (e.g., `TenorType::Month`).
/// * `create_point` - A factory function to create new points of type `T`.
///
/// # Returns
///
/// A list of tenors that were interpolated (newly created).
///
/// # Errors
///
/// Returns an error if:
/// - All partition elements are missing for a cover (nothing to anchor the interpolation).
///
/// # Example
///
/// ```
/// use tenors::{Tenor, TenorType};
/// use tenors::interpolation::{TenorPoint, interpolate_from_covers};
///
/// let mut curve = vec![
///     TenorPoint::new("F25".parse().unwrap(), 100.0),
///     TenorPoint::new("G25".parse().unwrap(), 105.0),
///     TenorPoint::new("1Q25".parse().unwrap(), 103.33),
/// ];
///
/// let interpolated = interpolate_from_covers(
///     &mut curve,
///     TenorType::Month,
///     TenorPoint::new,
/// ).unwrap();
///
/// assert_eq!(interpolated.len(), 1);
/// ```
pub fn interpolate_from_covers<T: TenorValue>(
    curve: &mut Vec<T>,
    target_type: TenorType,
    create_point: impl Fn(Tenor, f64) -> T,
) -> Result<Vec<Tenor>, InterpolationError> {
    let mut interpolated = Vec::new();

    // Build a lookup map: tenor -> index in curve
    let mut tenor_index: HashMap<Tenor, usize> = HashMap::new();
    for (idx, point) in curve.iter().enumerate() {
        tenor_index.insert(point.tenor().clone(), idx);
    }

    // Collect covers (tenors coarser than target_type)
    let covers: Vec<(Tenor, f64)> = curve
        .iter()
        .filter(|p| p.tenor().get_tenor_type() > target_type)
        .map(|p| (p.tenor().clone(), p.value()))
        .collect();

    // Process each cover
    for (cover_tenor, cover_value) in covers {
        // Get partition elements at target granularity
        let partition = match cover_tenor.partition(target_type) {
            Some(p) => p,
            None => continue, // Shouldn't happen if tenor_type > target_type
        };

        let n = partition.len() as f64;

        // Separate into present and missing
        let mut present_sum = 0.0;
        let mut present_count = 0usize;
        let mut missing: Vec<Tenor> = Vec::new();

        for part_tenor in &partition {
            if let Some(&idx) = tenor_index.get(part_tenor) {
                present_sum += curve[idx].value();
                present_count += 1;
            } else {
                missing.push(part_tenor.clone());
            }
        }

        // If nothing is missing, skip (could optionally validate consistency here)
        if missing.is_empty() {
            continue;
        }

        // Compute the value for missing elements
        // If all are missing, use constant interpolation (flat at cover value)
        // Otherwise: sum(all) = n * cover_value
        //            sum(missing) = n * cover_value - sum(present)
        //            Each missing element gets: sum(missing) / count(missing)
        let missing_value = if present_count == 0 {
            // All missing: use constant function (flat at cover value)
            cover_value
        } else {
            let total_sum = n * cover_value;
            let missing_sum = total_sum - present_sum;
            missing_sum / missing.len() as f64
        };

        // Add missing points to curve
        for tenor in missing {
            let new_point = create_point(tenor.clone(), missing_value);
            tenor_index.insert(tenor.clone(), curve.len());
            curve.push(new_point);
            interpolated.push(tenor);
        }
    }

    Ok(interpolated)
}

/// Validates that all cover constraints are satisfied within tolerance.
///
/// For each coarser tenor in the curve, this function checks that the arithmetic
/// average of its partition elements equals the cover value (within tolerance).
///
/// # Arguments
///
/// * `curve` - The curve to validate.
/// * `target_type` - The granularity to validate against.
/// * `tolerance` - The maximum allowed difference between cover value and partition average.
///
/// # Returns
///
/// `Ok(())` if all constraints are satisfied, otherwise an error describing the inconsistency.
pub fn validate_curve<T: TenorValue>(
    curve: &[T],
    target_type: TenorType,
    tolerance: f64,
) -> Result<(), InterpolationError> {
    // Build a lookup map
    let tenor_map: HashMap<&Tenor, f64> = curve.iter().map(|p| (p.tenor(), p.value())).collect();

    // Check each cover
    for point in curve.iter() {
        let tenor = point.tenor();
        if tenor.get_tenor_type() <= target_type {
            continue;
        }

        let partition = match tenor.partition(target_type) {
            Some(p) => p,
            None => continue,
        };

        // Check if all partition elements exist
        let values: Vec<f64> = partition
            .iter()
            .filter_map(|t| tenor_map.get(t).copied())
            .collect();

        if values.len() != partition.len() {
            // Not all partition elements present, skip validation
            continue;
        }

        let partition_avg = values.iter().sum::<f64>() / values.len() as f64;
        let cover_value = point.value();

        if (partition_avg - cover_value).abs() > tolerance {
            return Err(InterpolationError::InconsistentCurve {
                cover: tenor.clone(),
                cover_value,
                partition_avg,
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolate_single_missing_month() {
        // F25=100, G25=105, 1Q25=103.33 → H25 should be ~104.99
        let mut curve = vec![
            TenorPoint::new("F25".parse().unwrap(), 100.0),
            TenorPoint::new("G25".parse().unwrap(), 105.0),
            TenorPoint::new("1Q25".parse().unwrap(), 103.33),
        ];

        let interpolated =
            interpolate_from_covers(&mut curve, TenorType::Month, TenorPoint::new)
                .unwrap();

        assert_eq!(interpolated.len(), 1);
        assert_eq!(interpolated[0], "H25".parse::<Tenor>().unwrap());

        // Find the interpolated point
        let h25 = curve
            .iter()
            .find(|p| p.tenor == "H25".parse().unwrap())
            .unwrap();
        // 3 * 103.33 - 100 - 105 = 104.99
        assert!((h25.value - 104.99).abs() < 0.01);
    }

    #[test]
    fn test_interpolate_multiple_missing_months() {
        // 1Q25=100, only F25=90 given → G25 and H25 should each be 105
        let mut curve = vec![
            TenorPoint::new("F25".parse().unwrap(), 90.0),
            TenorPoint::new("1Q25".parse().unwrap(), 100.0),
        ];

        let interpolated =
            interpolate_from_covers(&mut curve, TenorType::Month, TenorPoint::new)
                .unwrap();

        assert_eq!(interpolated.len(), 2);

        // Both G25 and H25 should have value 105 (3*100 - 90 = 210, 210/2 = 105)
        let g25 = curve
            .iter()
            .find(|p| p.tenor == "G25".parse().unwrap())
            .unwrap();
        let h25 = curve
            .iter()
            .find(|p| p.tenor == "H25".parse().unwrap())
            .unwrap();

        assert!((g25.value - 105.0).abs() < 0.01);
        assert!((h25.value - 105.0).abs() < 0.01);
    }

    #[test]
    fn test_interpolate_all_missing_uses_constant() {
        // 1Q25=100, no months given → all months should be 100 (constant interpolation)
        let mut curve = vec![TenorPoint::new("1Q25".parse().unwrap(), 100.0)];

        let interpolated =
            interpolate_from_covers(&mut curve, TenorType::Month, TenorPoint::new)
                .unwrap();

        // All 3 months of Q1 should be interpolated
        assert_eq!(interpolated.len(), 3);

        // All should have the cover value (constant interpolation)
        let f25 = curve
            .iter()
            .find(|p| p.tenor == "F25".parse().unwrap())
            .unwrap();
        let g25 = curve
            .iter()
            .find(|p| p.tenor == "G25".parse().unwrap())
            .unwrap();
        let h25 = curve
            .iter()
            .find(|p| p.tenor == "H25".parse().unwrap())
            .unwrap();

        assert!((f25.value - 100.0).abs() < 0.01);
        assert!((g25.value - 100.0).abs() < 0.01);
        assert!((h25.value - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_interpolate_nothing_to_do() {
        // All months present, nothing to interpolate
        let mut curve = vec![
            TenorPoint::new("F25".parse().unwrap(), 100.0),
            TenorPoint::new("G25".parse().unwrap(), 105.0),
            TenorPoint::new("H25".parse().unwrap(), 110.0),
            TenorPoint::new("1Q25".parse().unwrap(), 105.0),
        ];

        let interpolated =
            interpolate_from_covers(&mut curve, TenorType::Month, TenorPoint::new)
                .unwrap();

        assert!(interpolated.is_empty());
    }

    #[test]
    fn test_validate_consistent_curve() {
        // Curve where quarter equals average of months
        let curve = vec![
            TenorPoint::new("F25".parse().unwrap(), 100.0),
            TenorPoint::new("G25".parse().unwrap(), 105.0),
            TenorPoint::new("H25".parse().unwrap(), 110.0),
            TenorPoint::new("1Q25".parse().unwrap(), 105.0), // (100+105+110)/3 = 105
        ];

        let result = validate_curve(&curve, TenorType::Month, 0.01);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_inconsistent_curve() {
        // Curve where quarter doesn't match average of months
        let curve = vec![
            TenorPoint::new("F25".parse().unwrap(), 100.0),
            TenorPoint::new("G25".parse().unwrap(), 105.0),
            TenorPoint::new("H25".parse().unwrap(), 110.0),
            TenorPoint::new("1Q25".parse().unwrap(), 120.0), // Wrong! Should be 105
        ];

        let result = validate_curve(&curve, TenorType::Month, 0.01);
        assert!(matches!(
            result,
            Err(InterpolationError::InconsistentCurve { .. })
        ));
    }

    #[test]
    fn test_validate_skips_incomplete_partitions() {
        // Curve with missing months - validation should pass (skip)
        let curve = vec![
            TenorPoint::new("F25".parse().unwrap(), 100.0),
            TenorPoint::new("G25".parse().unwrap(), 105.0),
            // H25 missing
            TenorPoint::new("1Q25".parse().unwrap(), 120.0),
        ];

        let result = validate_curve(&curve, TenorType::Month, 0.01);
        assert!(result.is_ok()); // Skips validation because partition incomplete
    }

    #[test]
    fn test_interpolate_year_to_quarters() {
        // Cal25=100, 1Q25=90, 2Q25=95 → 3Q25 and 4Q25 should be 107.5 each
        let mut curve = vec![
            TenorPoint::new("1Q25".parse().unwrap(), 90.0),
            TenorPoint::new("2Q25".parse().unwrap(), 95.0),
            TenorPoint::new("Cal25".parse().unwrap(), 100.0),
        ];

        let interpolated =
            interpolate_from_covers(&mut curve, TenorType::Quarter, TenorPoint::new)
                .unwrap();

        assert_eq!(interpolated.len(), 2);

        // 4 * 100 - 90 - 95 = 215, 215/2 = 107.5
        let q3 = curve
            .iter()
            .find(|p| p.tenor == "3Q25".parse().unwrap())
            .unwrap();
        let q4 = curve
            .iter()
            .find(|p| p.tenor == "4Q25".parse().unwrap())
            .unwrap();

        assert!((q3.value - 107.5).abs() < 0.01);
        assert!((q4.value - 107.5).abs() < 0.01);
    }

    #[test]
    fn test_interpolate_halfyear_to_months() {
        // 1H25=100, F25=90, G25=95, H25=100, J25=105, K25=110 → M25 should be 100
        let mut curve = vec![
            TenorPoint::new("F25".parse().unwrap(), 90.0),
            TenorPoint::new("G25".parse().unwrap(), 95.0),
            TenorPoint::new("H25".parse().unwrap(), 100.0),
            TenorPoint::new("J25".parse().unwrap(), 105.0),
            TenorPoint::new("K25".parse().unwrap(), 110.0),
            TenorPoint::new("1H25".parse().unwrap(), 100.0),
        ];

        let interpolated =
            interpolate_from_covers(&mut curve, TenorType::Month, TenorPoint::new)
                .unwrap();

        assert_eq!(interpolated.len(), 1);

        // 6 * 100 - (90+95+100+105+110) = 600 - 500 = 100
        let m25 = curve
            .iter()
            .find(|p| p.tenor == "M25".parse().unwrap())
            .unwrap();
        assert!((m25.value - 100.0).abs() < 0.01);
    }
}
