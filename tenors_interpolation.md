# Tenor Interpolation for Forward Curves

## Problem Statement

Forward curves often contain market data quoted at mixed granularities. For example, a curve might include:

| Tenor | Value |
|-------|-------|
| F25   | 100   |
| G25   | 105   |
| 1Q25  | 103.33 |

Given these observations, we want to infer the missing month **H25** (March 2025) using the constraint that **1Q25** covers **F25**, **G25**, and **H25**.

## Approach

### Trait-Based Design

Define a trait for objects that carry tenor-value pairs:

```rust
trait TenorValue {
    fn get_tenor(&self) -> Tenor;
    fn get_value(&self) -> f64;
    fn set_value(&mut self, value: f64);
}
```

A forward curve is represented as a `Vec<T>` where `T: TenorValue`.

### Interpolation via Cover Constraints

For any coarser tenor (e.g., a quarter) that **covers** a set of finer tenors (e.g., months), the following arithmetic-average constraint should hold:

```
value(cover) = average(value(partition_elements))
```

The interpolation algorithm:

1. For each coarser tenor in the curve, identify the finer tenors it covers using `Tenor::partition()`.
2. Check which partition elements are present in the curve and which are missing.
3. If some partition elements are missing, solve for their values using the average constraint:
   - If *n* elements are missing and *k* are present: `sum(missing) = n * value(cover) - sum(present)`
   - Distribute the residual equally among the missing elements (flat interpolation).

---

## Implementation Plan

### Step 1: Create a New Module

Create `src/interpolation.rs` and add `pub mod interpolation;` to `src/lib.rs`.

This keeps interpolation logic separate from core tenor types while allowing access to `Tenor`, `TenorType`, and related items.

### Step 2: Define the `TenorValue` Trait

```rust
// src/interpolation.rs

use crate::{Tenor, TenorType};

/// Trait for objects that associate a tenor with a numeric value.
pub trait TenorValue {
    fn tenor(&self) -> &Tenor;
    fn value(&self) -> f64;
    fn set_value(&mut self, value: f64);
}
```

### Step 3: Provide a Default Implementation Struct

For convenience, provide a simple struct that implements the trait:

```rust
/// A simple tenor-value pair.
#[derive(Debug, Clone)]
pub struct TenorPoint {
    pub tenor: Tenor,
    pub value: f64,
}

impl TenorValue for TenorPoint {
    fn tenor(&self) -> &Tenor { &self.tenor }
    fn value(&self) -> f64 { self.value }
    fn set_value(&mut self, value: f64) { self.value = value; }
}
```

### Step 4: Define Error Types

Add interpolation-specific errors:

```rust
#[derive(Debug, thiserror::Error)]
pub enum InterpolationError {
    #[error("no cover constraint available for tenor {0}")]
    NoCoverConstraint(Tenor),

    #[error("inconsistent curve: cover {cover} has value {cover_value}, but partition average is {partition_avg}")]
    InconsistentCurve {
        cover: Tenor,
        cover_value: f64,
        partition_avg: f64,
    },

    #[error("cannot interpolate: all partition elements missing for {0}")]
    AllPartitionElementsMissing(Tenor),
}
```

### Step 5: Implement the Core Interpolation Function

```rust
/// Interpolates missing finer-granularity tenors using cover constraints.
///
/// For each coarser tenor in the curve, checks if its partition elements
/// exist. Missing elements are filled in using the arithmetic average constraint.
///
/// Returns the tenors that were interpolated.
pub fn interpolate_from_covers<T: TenorValue + Clone>(
    curve: &mut Vec<T>,
    target_type: TenorType,
    create_point: impl Fn(Tenor, f64) -> T,
) -> Result<Vec<Tenor>, InterpolationError> {
    // Implementation details below
}
```

**Algorithm outline:**

1. Build a `HashMap<Tenor, usize>` for O(1) lookup of existing tenors.
2. Sort curve by granularity using `Tenor::cmp_by_granularity`.
3. Iterate over tenors coarser than `target_type`:
   - Call `tenor.partition(target_type)` to get the finer tenors.
   - Partition elements into present vs. missing.
   - If all present: optionally validate consistency.
   - If some missing: compute residual and distribute equally.
   - Insert new points using `create_point`.
4. Return list of interpolated tenors.

### Step 6: Add Consistency Validation

```rust
/// Validates that all cover constraints are satisfied within tolerance.
pub fn validate_curve<T: TenorValue>(
    curve: &[T],
    tolerance: f64,
) -> Result<(), InterpolationError> {
    // For each coarser tenor, verify average of partition equals cover value
}
```

### Step 7: Write Unit Tests

Add tests in `src/interpolation.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolate_single_missing_month() {
        // F25=100, G25=105, 1Q25=103.33 → H25=105
        let mut curve = vec![
            TenorPoint { tenor: "F25".parse().unwrap(), value: 100.0 },
            TenorPoint { tenor: "G25".parse().unwrap(), value: 105.0 },
            TenorPoint { tenor: "1Q25".parse().unwrap(), value: 103.33 },
        ];

        let interpolated = interpolate_from_covers(
            &mut curve,
            TenorType::Month,
            |t, v| TenorPoint { tenor: t, value: v },
        ).unwrap();

        assert_eq!(interpolated.len(), 1);
        // H25 should be ~105 (3 * 103.33 - 100 - 105 = 104.99)
    }

    #[test]
    fn test_interpolate_multiple_missing() {
        // 1Q25=100, only F25=90 given → G25=H25=105
    }

    #[test]
    fn test_validate_consistent_curve() {
        // All constraints satisfied
    }

    #[test]
    fn test_validate_inconsistent_curve() {
        // Cover value doesn't match partition average
    }
}
```

### Step 8: Add an Example

Create `examples/interpolation.rs`:

```rust
use tenors::{Tenor, TenorType};
use tenors::interpolation::{TenorPoint, interpolate_from_covers};

fn main() {
    let mut curve = vec![
        TenorPoint { tenor: "F25".parse().unwrap(), value: 100.0 },
        TenorPoint { tenor: "G25".parse().unwrap(), value: 105.0 },
        TenorPoint { tenor: "1Q25".parse().unwrap(), value: 103.33 },
    ];

    println!("Before interpolation:");
    for p in &curve {
        println!("  {} = {}", p.tenor, p.value);
    }

    let added = interpolate_from_covers(
        &mut curve,
        TenorType::Month,
        |t, v| TenorPoint { tenor: t, value: v },
    ).unwrap();

    println!("\nInterpolated tenors: {:?}", added);
    println!("\nAfter interpolation:");
    for p in &curve {
        println!("  {} = {:.2}", p.tenor, p.value);
    }
}
```

### Step 9: Documentation

Add rustdoc comments to all public items and include module-level documentation in `src/interpolation.rs` explaining the use case and constraints.

---

## File Checklist

| File | Action |
|------|--------|
| `src/lib.rs` | Add `pub mod interpolation;` |
| `src/interpolation.rs` | New file with trait, struct, errors, functions, tests |
| `examples/interpolation.rs` | New example demonstrating usage |

---

## Open Questions

1. **Weighted averages**: Should we support day-count weighting instead of simple arithmetic averages? (e.g., months have different lengths)

2. **Multiple passes**: If the curve has nested covers (Year → Quarter → Month), should interpolation cascade automatically?

3. **Conflict resolution**: If a finer tenor exists but contradicts the cover constraint, should we error or overwrite?
