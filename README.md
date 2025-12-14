# Tenors

> **Warning:** This library is not production ready. The API may change without notice.

A Rust crate for working with financial tenors (time periods) commonly used in financial contracts.

## Features

- Type-safe tenor arithmetic with overflow protection
- Support for various tenor types: HalfMonth, Month, Quarter, HalfYear, Year
- Futures contract code parsing (e.g., "F24", "F2024", "DEC 32")
- String parsing via `FromStr` for multiple formats (e.g., "1Q24", "24Q1", "1H24", "Cal24")
- Conversion between tenor types (covering and partitioning)
- Forward curve interpolation using cover constraints
- Integration with chrono for date calculations
- Serialization support via serde

## Quick Start

```rust
use tenors::{Tenor, TenorType, TenorDuration};

// Create tenors from year and month
let jan_2024 = Tenor::month_from_ym0(2024, 0);

// Parse futures codes
let tenor = Tenor::month_from_futures_code("F24").unwrap();

// Safe arithmetic with checked operations
let next_month = jan_2024.checked_add(1).unwrap();

// Add durations
let apr_2024 = (&jan_2024 + TenorDuration::new(TenorType::Month) * 3).unwrap();

// Get date boundaries
let first = jan_2024.first_day();
let last = jan_2024.last_day();

// Parse from string
let quarter: Tenor = "1Q24".parse().unwrap();
let half_year: Tenor = "1H24".parse().unwrap();
let year: Tenor = "Cal24".parse().unwrap();
```

## Interpolation

The `interpolation` module provides forward curve interpolation using cover constraints:

```rust
use tenors::{Tenor, TenorType};
use tenors::interpolation::{TenorPoint, interpolate_from_covers};

let mut curve = vec![
    TenorPoint::new("F25".parse().unwrap(), 100.0),
    TenorPoint::new("G25".parse().unwrap(), 105.0),
    TenorPoint::new("1Q25".parse().unwrap(), 103.33),
];

// Interpolate missing H25 (March) using the quarter constraint
let interpolated = interpolate_from_covers(
    &mut curve,
    TenorType::Month,
    |t, v| TenorPoint::new(t, v),
).unwrap();
```

## Documentation

Run `cargo doc --open` to view the full API documentation.

## Examples

See the `examples/` folder for more comprehensive usage examples.
