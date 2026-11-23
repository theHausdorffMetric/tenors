# Changelog

All notable changes to this project are documented here. The format is based
on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html) — pre-1.0,
breaking changes bump the minor version.

## [Unreleased]

## [0.3.0] - 2026-11-23

Breaking: the date type, the serde output, the short-year rule and the
interpolation error enum change.

### Fixed
- **Parsing no longer panics on huge years.** `"F9223372036854775807"`,
  `"1Q9223372036854775807"` and the same strings through serde panicked with
  "tenor offset overflow"; every parser now returns
  `TenorsError::UnparsableTenorString`. `parse_with_reference` with a
  reference at the edge of `isize` is an error too.
- Plain constructors (`year_from_y(isize::MIN)`, `half_month_from_ymh0` with
  a huge month) panicked with an arithmetic-overflow message in debug and
  wrapped silently in release; they now always panic with
  "tenor offset overflow". The `try_*` constructors return `None` on
  overflow.
- **Interpolation rejects NaN and infinite values.** A NaN cover used to fill
  its months with NaN and pass `validate_curve`, because `NaN > tolerance` is
  false. `interpolate_from_covers` and `validate_curve` now return the new
  `InterpolationError::NonFiniteValue { tenor, value }`, and a NaN
  `tolerance` no longer passes every cover.
- **A failed `interpolate_from_covers` leaves the curve unchanged.** It used
  to return the error with the points it had added so far still on the
  curve; they are now removed again.

### Changed
- **`chrono` replaced by `jiff`** (1.0, no default features; chrono is
  unmaintained, RUSTSEC-2026-0543). `first_day()` / `last_day()` return
  `Option<jiff::civil::Date>` — `None` outside years -9999..=9999;
  `half_month_from_date` takes a `jiff::civil::Date` by value and
  `From<&impl Datelike>` is now `From<jiff::civil::Date>`.
- **Serde writes the full year** (`"F2024"`, `"1Q2024"`, `"2HZ2099"`), so a
  stored tenor no longer changes meaning when the century turns (a 0.2
  `"F99"` written in 2099 read back as 2199 in 2100). Short-year strings
  written by 0.2 still deserialize, against the current year.
- **Short years use a sliding window** instead of the current century or
  decade: two digits resolve to the matching year in
  `reference - 50 ..= reference + 49`, one digit to
  `reference - 1 ..= reference + 8`. In 2029 `F0` is 2030 (was 2020); in
  2026 `F99` is 1999 (was 2099) and `F4` is 2034 (was 2024). `Display`
  prints two digits exactly inside the two-digit window, so it stays the
  inverse of `FromStr`.

### Added
- The alternate form `{:#}` prints the full year and does not read the clock.
- `Tenor::format_with_reference(year)`, the inverse of
  `parse_with_reference`.
- `InterpolationError::NonFiniteValue`.

### Packaging
- CI on GitHub Actions: fmt, clippy, tests, examples, docs and
  `cargo package` on stable; tests on the `rust-version` toolchain;
  `cargo audit --deny warnings`; weekly as well as on pushes and PRs.

## [0.2.0] - 2026-09-13

Everything from the 2026-09 code review (`CODE_REVIEW-2026-09.md` in the
repository), in one release. Breaking: several signatures and the serde
format change.

### Fixed
- **`cover()` was wrong for tenors before 1970**: it truncated instead of
  flooring, so December 1969 covered the year 1970. Now Euclidean division,
  like the accessors.
- **The 15th of a month belongs to the first half-month**, as the docs,
  `first_day`/`last_day` and Platts' half-month cycles (1st–15th, 16th–end)
  say. Converting a date put the 15th into the second half.
- **Parsing non-ASCII input no longer panics** (`"C€".parse::<Tenor>()`,
  `month_from_futures_code("€")`, `cme_tenor_parser("ÄÄ32")` used to index
  into the middle of a character).
- **`Tenor ± TenorDuration` is checked**: overflow returns
  `TenorsError::ArithmeticOverflow` instead of wrapping in release builds.
- **Interpolation is order-independent**: covers are processed finest-first
  so the tightest constraint fills first, and a curve whose covers contradict
  each other is reported as `InterpolationError::InconsistentCurve` instead
  of being returned with the contradiction inside. Previously the result
  depended on the input order.
- Out-of-range constructor arguments **carry** instead of wrapping:
  `Tenor::month_from_ym0(2024, 12)` is January 2025, not January 2024.

### Changed
- **One year rule for every parser**: two-digit years resolve against the
  current century and one-digit month codes (`F7`) against the current
  decade — the rule `FromStr` used — now also in `month_from_futures_code`
  and `cme_tenor_parser`, which had a fixed 2000 pivot. One tokenizer
  behind all three.
- **`Display` and `FromStr` are inverses for every type.** Half-months
  parse (`1HF24`, `2HF2024`); a bare 4-digit year parses (`2024`, next to
  `C24`, `Cal24`, `C2024`, `Cal2024`); tenors outside the current century
  print a 4-digit year (`F1999`) so they round-trip too.
- **Serde format is the canonical string** (`"F24"`, `"1Q24"`, `"2024"`,
  `"1HF24"`) instead of `{"tenor_type":…,"epoch_offset":…}`. No
  compatibility path: nothing persisted the old form.
- `first_day()` / `last_day()` return `Option<NaiveDate>` — `None` for years
  outside chrono's range instead of a panic.
- `year()` returns `isize` (it could never be `None`); `month_from_ym0`
  takes `month0: isize` like `quarter_from_yq0`; `get_tenor_type()` is
  `tenor_type()` (old name kept, deprecated).
- `TenorsError::IncompatibleTenors` carries the two `TenorType`s instead of
  their debug strings.
- `interpolate_from_covers` takes a tolerance and can now actually fail;
  the never-returned `NoCoverConstraint` and `AllPartitionElementsMissing`
  variants are gone.
- `Tenor`, `TenorDuration` and `TenorPoint` are `Copy`; arithmetic works on
  values as well as references.

### Added
- `Tenor::year_from_y`, `half_year_from_yh0`, `half_month_from_ymh0`,
  `half_month_from_date`; strict `try_month_from_ym0` / `try_quarter_from_yq0`
  / `try_half_year_from_yh0` that reject out-of-range parts; `half0()`.
- `Tenor::parse_with_reference(s, year)` to resolve short years against a
  chosen year instead of the clock.
- `CHANGELOG.md`.

### Removed
- The `log` dependency (leftover debug output in `last_day`) and the
  `anyhow` dev-dependency.

### Packaging
- Repository moved to <https://github.com/theHausdorffMetric/tenors>;
  `rust-version = "1.98"` (the code already needed 1.88 for `if let`
  chains); `include` allowlist.

## [0.1.0] - 2025-12-14

First release.

[Unreleased]: https://github.com/theHausdorffMetric/tenors/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/theHausdorffMetric/tenors/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/theHausdorffMetric/tenors/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/theHausdorffMetric/tenors/releases/tag/v0.1.0
