# CLAUDE.md

Guidance for Claude Code when working in this repository.

## Project overview

A Rust library for financial tenors (half-months, months, quarters,
half-years, years) and the futures contract codes that name them.

## Architecture

- `Tenor { tenor_type, epoch_offset }` — offsets from a 1970 epoch in units
  of the tenor type; granularities in half-months (HalfMonth 1, Month 2,
  Quarter 6, HalfYear 12, Year 24). Accessors and `cover` use Euclidean
  division so pre-1970 tenors are correct.
- One tokenizer (`parse_tenor`) behind `FromStr`, `month_from_futures_code`
  and `cme_tenor_parser`; short years resolve against the clock (two digits
  → current century, one digit → current decade) or an explicit reference
  year. `Display` is the inverse for every type.
- Half-months split like Platts' cycles: 1st–15th, 16th–end.
- Serde: a tenor is its canonical string.
- `interpolation`: fills missing finer tenors from coarser covers,
  finest-first, and reports contradictory covers.

## Commands

```bash
cargo test                                   # unit tests (incl. round-trip sweeps)
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo doc --open
cargo run --example t1_general
cargo run --example interpolation
```

## Conventions

- Rust edition 2024, `rust-version` tracks current stable.
- Dependencies: `chrono`, `serde`, `thiserror`; dev: `serde_json`.
- Errors via `thiserror` (`TenorsError`, `InterpolationError`); operators on
  plain integers panic on overflow like `isize` does — use `checked_*` /
  `saturating_*` when that matters.
- Tests that involve short years derive their expectations from the same
  clock rule, never from a hard-coded year.
- `CODE_REVIEW-2026-09.md`, this file and `tenors_interpolation.md` are
  excluded from the published package.
