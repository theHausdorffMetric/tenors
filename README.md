# Tenors

> **Warning:** This library is not production ready. The API may change without notice.

A Rust crate for working with financial tenors — the half-months, months,
quarters, half-years and years that commodity and financial contracts are
quoted in — and the futures contract codes that name them.

## Features

- Five tenor types on one integer model: `HalfMonth`, `Month`, `Quarter`,
  `HalfYear`, `Year`, each a type plus an offset from a 1970 epoch.
- Every type prints and parses the same way: `1HF24`, `F24`, `1Q24`, `1H24`,
  `2024`. Futures codes with one-, two- or four-digit years (`F7`, `F24`,
  `F2024`), period-first and year-first quarters and half-years (`1Q24`,
  `24Q1`), `C24`/`Cal24` years, CME `DEC 32`.
- Short years follow the clock: two digits resolve against the current
  century, one digit against the current decade — or against a year you
  supply with `Tenor::parse_with_reference`.
- Cover and partition between granularities (`month.cover(Quarter)`,
  `year.partition(Month)`), calendar boundaries (`first_day`, `last_day`),
  half-months split like Platts' cycles: 1st–15th and 16th–end.
- Checked and saturating arithmetic; adding a `TenorDuration` of a coarser
  type to a finer tenor (`month + 3 quarters`).
- Forward-curve interpolation from cover constraints (`interpolation`),
  finest-first, with consistency validation.
- Serde support: a tenor serialises as its canonical string.

## Quick start

```rust
use tenors::{Tenor, TenorType, TenorDuration};

let jan_2024 = Tenor::month_from_ym0(2024, 0);
let tenor = Tenor::month_from_futures_code("F24").unwrap();
assert_eq!(jan_2024, tenor);

// checked and saturating arithmetic on the offset
let feb_2024 = jan_2024.checked_add(1).unwrap();
// durations of a coarser type
let apr_2024 = (jan_2024 + TenorDuration::new(TenorType::Quarter)).unwrap();

// calendar boundaries (None only outside chrono's year range)
assert_eq!(jan_2024.first_day().unwrap().to_string(), "2024-01-01");
assert_eq!(jan_2024.last_day().unwrap().to_string(), "2024-01-31");

// every type round-trips through its string
let q: Tenor = "1Q24".parse().unwrap();
assert_eq!(q.to_string(), "1Q24");
let hm: Tenor = "2HF24".parse().unwrap();       // 16–31 January 2024
let y: Tenor = "2024".parse().unwrap();         // also "Cal24", "C2024"
assert_eq!(y.partition(TenorType::Quarter).unwrap().len(), 4);
let _ = (feb_2024, hm);
```

## Interpolation

```rust
use tenors::TenorType;
use tenors::interpolation::{TenorPoint, interpolate_from_covers};

let mut curve = vec![
    TenorPoint::new("F25".parse().unwrap(), 100.0),
    TenorPoint::new("G25".parse().unwrap(), 105.0),
    TenorPoint::new("1Q25".parse().unwrap(), 103.33),
];
// fills H25 from the quarter's average constraint; errors if covers contradict each other
let added = interpolate_from_covers(&mut curve, TenorType::Month, 1e-9, TenorPoint::new).unwrap();
assert_eq!(added.len(), 1);
```

## Documentation

`cargo doc --open` for the API; `examples/` for longer walkthroughs;
`CHANGELOG.md` for what changed.
