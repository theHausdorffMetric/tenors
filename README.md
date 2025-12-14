# Tenors

A Rust crate for working with financial tenors (time periods) commonly used in financial contracts.

## Features

- Type-safe tenor arithmetic with overflow protection
- Support for various tenor types: HalfMonth, Month, Quarter, HalfYear, Year
- Futures contract code parsing (e.g., "F24" for January 2024)
- Conversion between tenor types (covering and partitioning)
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
```

## Documentation

Run `cargo doc --open` to view the full API documentation.

## Examples

See the `examples/` folder for more comprehensive usage examples.
