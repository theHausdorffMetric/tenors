# tenors — code review, 2026-09-13 (pre-0.1.1)

**Scope:** the whole crate at commit `6013a09` (crates.io 0.1.0, last touched
2025-12-14): `src/lib.rs` (1,643 lines incl. 33 tests), `src/interpolation.rs`
(492 lines incl. 8 tests), both examples, manifest, README, `CLAUDE.md`,
`tenors_interpolation.md`. Method: full read, then every suspected defect
was reproduced with a throw-away probe program before it was written down —
the *Evidence* lines below are that program's output.

**Gate at review time (rustc 1.98.1):** `cargo fmt --check` clean, `cargo
clippy --all-targets -- -D warnings` clean, 41 tests pass, no `unsafe`, four
runtime dependencies, `cargo audit`: no vulnerabilities, one unsound-crate
warning in the dev-only `anyhow` (RUSTSEC-2026-0190).

**Verdict:** a small, well-shaped model — offsets from a 1970 epoch in units of
the tenor type, Euclidean division in the accessors, docs on every public
item — with a handful of real defects at the edges: one wrong result for
pre-1970 tenors, one calendar-boundary contradiction, three parser panics on
non-ASCII input, an order-dependent interpolation, and unchecked arithmetic
on the one operator that already returns a `Result`. All of those are fixable
without breaking the API. The larger issues — display and parse not being
inverses, wall-clock-dependent parsing, three parsers for one format — are
0.2.0 material and need decisions from Dan (§4).

IDs: **C** correctness, **R** robustness, **D** design/API, **A** hygiene and
packaging, **T** tests. Severity: ● must fix, ◐ should fix, ○ nice to have.

## 1. Correctness

### C1 ● `cover()` is wrong for tenors before the epoch
`src/lib.rs:372` computes the covering offset with truncating integer
division (`/`). Negative offsets truncate toward zero instead of the floor,
so any tenor before 1970 maps to the wrong cover.
*Evidence:* `Dec 1969 (Month, -1).cover(Year)` → `Year 0` = 1970;
`4Q69.cover(Year)` → 1970, `.cover(HalfYear)` → 1H70. `Jan 1970` → 0, correct.
*Fix:* `(self.tenor_type.granularity() * self.epoch_offset).div_euclid(target.granularity())`
— the accessors `year()`, `month0()`, `quarter0()` already use `div_euclid`;
`cover` is the odd one out. Add tests with negative offsets for every type.

### C2 ◐ `month_from_ym0` / `quarter_from_yq0` silently wrap out-of-range parts
`month0 % 12` (line 270) turns `month_from_ym0(2024, 12)` into January **2024**,
not January 2025 and not an error; `quarter_from_yq0` does the same with a
truncating `%` on `isize`, so negative quarters produce a carry into the
previous year while months cannot be negative at all (`usize`). Both carry a
`// todo error/range checking`.
*Evidence:* `month_from_ym0(2024, 12)` → `F24`.
*Fix (decision, §4-Q4):* either normalise with carry (`year + month0 / 12`,
`quarter0.div_euclid(4)`) or reject (`Option`). Add `try_*` constructors if
the plain ones stay total.

### C3 ● The 15th of the month belongs to both half-months
`From<&Datelike>` (line 204) puts `day() > 14` into the second half, i.e. the
15th is second-half. `first_day()`/`last_day()` (lines 402–474) and the
`TenorType::HalfMonth` doc define the first half as the 1st–15th and the
second as the 16th–end. So `Tenor::from(15 March)` is a half-month whose own
`first_day()` is the 16th.
*Evidence:* `2024-03-15` → `2HH24`, whose `first_day()` = 2024-03-16;
the first half's `last_day()` = 2024-03-15.
*Fix:* `day() > 15` in `From`; `test_from1` currently enshrines the bug
(`1970-01-15` → offset 1) and must flip to offset 0. Add a boundary test for
the 14th/15th/16th of a 28-, 30- and 31-day month.

### C4 ● `Add/Sub<TenorDuration>` are unchecked; `ArithmeticOverflow` is never returned
Lines 792–828 use plain `+`, `-`, `*` (panic in debug, silent wrap in
release) while the `isize` operators use `checked_*` + `expect`. The error
enum has an `ArithmeticOverflow` variant that no code path constructs, and
these two operators are the only ones that already return `Result`.
*Evidence:* `Month(isize::MAX) + 1 Year` → panic "attempt to add with overflow".
*Fix:* `checked_mul`/`checked_add`/`checked_sub` and
`Err(TenorsError::ArithmeticOverflow)` — non-breaking, the signature is
already `Result`.

### C5 ● Interpolation result depends on the input order
`interpolate_from_covers` (`src/interpolation.rs:167–223`) walks the covers in
curve order. With nested covers (year → half-year → quarter → month) a coarse
cover processed first fills every month from the yearly average; the finer
covers then find all their months present and are skipped, so their
constraints are violated. `tenors_interpolation.md` §"Algorithm outline" step
2 says "sort curve by granularity"; the implementation never does.
*Evidence:* F25=100, G25=101, H25=102, 2Q25=104, 2H25=110, Cal25=106.25 —
in that order → J/K/M25 = 104, N…Z25 = 110, `validate_curve` OK. With Cal25
first → every missing month = 108, `validate_curve` → "cover 2Q25 has value
104, but partition average is 108".
*Fix:* sort the collected covers finest-first (`cmp_by_granularity`) so the
tightest constraint fills first and coarser covers distribute over what is
left; then run the consistency check and surface a violation as
`InconsistentCurve` instead of returning a curve that contradicts itself
(see D4). Add the nested-cover test in both orders.

## 2. Robustness

### R1 ● Three parsers panic on non-ASCII input
Byte-index slicing without char-boundary checks: `month_from_futures_code`
(`fc[1..3]`, `fc[1..5]`, evaluated eagerly inside the tuple even when the
first char is not a futures code), `cme_tenor_parser` (`[0..3]`, `[4..6]`),
`parse_year_format` (`remainder[..2]`). A `FromStr` that panics on a string
is a bug for a library that will see user input.
*Evidence:* `"C€".parse::<Tenor>()` → panic "end byte index 2 is not a char
boundary"; `Tenor::month_from_futures_code("€")` → panic;
`cme_tenor_parser("ÄÄ32")` → panic.
*Fix:* reject non-ASCII up front (`if !s.is_ascii() { return Err(...) }`) or use
`s.get(a..b)` / `as_bytes()`; add tests with multibyte input for every parser.

### R2 ◐ `first_day()` / `last_day()` panic outside chrono's year range
`from_ymd_opt(...).unwrap()` and `year().unwrap() as i32`: a `Tenor::new(Year,
300_000)` panics in `first_day()`. Reachable, since `new` is public and
unchecked.
*Fix:* 0.2.0: return `Option<NaiveDate>`; 0.1.1: document the panic.

### R3 ○ Operator overflow panics are a choice, but undocumented
`&tenor + isize` etc. panic via `expect("isize over/underflow in tenor!")`.
Acceptable (matches integer semantics), but the README's "overflow
protection" and the operator docs should say: *panics on overflow, use
`checked_*` / `saturating_*`*.

## 3. Design and API (0.2.0)

### D1 ◐ `Display` and `FromStr` are not inverses
- `Year` prints `2024`; only `C2024` / `Cal24` parse. *Evidence:* `"2024".parse` → error.
- `HalfMonth` prints `1HF24`; nothing parses it. *Evidence:* `"1HF24".parse` → error.
- Two-digit years: `Jan 1999` prints `F99`, which re-parses as 2099.
Month, Quarter and HalfYear round-trip within the current century.
*Fix:* define the canonical string per type, add the HalfMonth parser, accept
bare 4-digit years (or print `Cal2024`), and pin it with a round-trip
property test over all types and a wide year range.

### D2 ◐ Parsing depends on the wall clock, three different ways
`FromStr` resolves 2-char codes (`F7`) against the *current decade* and
3-char codes (`F24`) against the *current century* via `Utc::now()`;
`month_from_futures_code` uses a fixed `2000 +`; `cme_tenor_parser` a fixed
`2000 +`. The same string means different tenors on different days, the
tests bake the clock into their expectations, and two entry points disagree
in 2100. *Fix:* one documented rule implemented once — a fixed 2000 pivot
(the exchange convention) or an explicit reference year parameter
(`Tenor::parse_with_reference(s, year)`); drop or gate the 2-char format.

### D3 ○ Three implementations of the month-code format
`month_from_futures_code`, `parse_month_futures`, `cme_tenor_parser` each
tokenise on their own. Consolidate on one tokenizer behind `FromStr`; keep
the named functions as thin wrappers.

### D4 ◐ `interpolate_from_covers` documents an error that cannot happen
Its `# Errors` section promises an error when all partition elements are
missing; the code interpolates flat instead and the function never returns
`Err`. `AllPartitionElementsMissing` is a dead variant kept "for backwards
compatibility" with `#[allow(dead_code)]` on a public enum. With C5 fixed the
`Result` earns its keep: return `InconsistentCurve` when constraints
conflict; delete the dead variant in 0.2.0.

### D5 ○ Serde format exposes the epoch encoding
`Tenor` serialises as `{"tenor_type":"Month","epoch_offset":648}`. Stable,
but opaque to humans and coupled to `TENOR_EPOCH`. Once D1 gives a canonical
string, serialising as that string (with a compatibility deserialiser for the
struct form) is the friendlier wire format. Breaking for stored data → 0.2.0
at the earliest, and only if anything persists tenors.

### D6 ○ `Tenor` and `TenorDuration` should be `Copy`
Both are two `Copy` fields. Deriving `Copy` (non-breaking) removes the
`&tenor + 1` / `.clone()` noise throughout the API and the interpolation
module.

### D7 ○ Small API warts, for 0.2.0
`year()` returns `Option` but can never be `None` (the guard compares
against the coarsest type); `month_from_ym0(isize, usize)` vs
`quarter_from_yq0(isize, isize)`; `From<&Datelike>` silently chooses
HalfMonth granularity — a named `half_month_from_date` reads better;
`get_tenor_type()` → `tenor_type()` per Rust convention.

## 4. Decisions needed from Dan

1. **Century rule (D2):** fixed 2000 pivot, sliding window around a supplied
   reference year, or keep the clock? Keep the 2-char `F7` format at all?
2. **Canonical Year string (D1):** `Cal2024` (parses today) or bare `2024`
   (what `Display` prints today)?
3. **The 15th (C3):** confirm first half = 1st–15th as the docs say.
4. **Out-of-range parts (C2):** carry (`month0 = 12` → next January) or error?
5. **Serde format (D5):** does anything persist `Tenor` JSON today?
6. **Fallible calendar functions (R2):** accept the 0.2.0 signature change?

## 5. Hygiene, packaging, tests

- **A1** ◐ Five `debug!` calls left in `last_day()` from a debugging session;
  remove them, and with them the `log` dependency.
- **A2** ○ Docs: "FUTERESCODES", "Approprite"; `CLAUDE.md` lists `regex` and
  `env_logger` dependencies that do not exist and calls the suite
  comprehensive; the README's "overflow protection" (see R3). The README's
  "not production ready" warning is accurate and should stay through 0.1.x.
- **A3** ○ The model's base unit is the half-month; anything that does not
  divide 24 half-months (weeks, days) cannot join without changing the
  epoch arithmetic. Fine for the stated scope; worth one sentence in the docs.
- **A4** ● Manifest: no `rust-version`, yet the `if let` chains need ≥ 1.88 —
  declare `rust-version = "1.98"` (current stable). Replace `exclude` with an
  `include` allowlist (`src/**`, `examples/*.rs`, `/README.md`, `/LICENSE`;
  root files anchored) and keep this review, `CLAUDE.md` and the design note
  out. `repository` → `https://github.com/theHausdorffMetric/tenors`.
- **A5** ○ `cargo update` for the dev-only `anyhow` advisory.
- **T1** ◐ Test gaps: no test for `cover()` at all, none for
  `first_day`/`last_day`, none for negative offsets, none for `Display` of
  Month/Quarter/HalfYear/Year (only HalfMonth), none for the `TenorDuration`
  operators or `AddAssign` (the TODO list at `lib.rs:1001–1007` says so),
  none for multibyte input, none for nested-cover interpolation. Clock
  dependence in expectations (D2). No round-trip property test (D1).

## 6. Fix plan

**Phase 1 — 0.1.1, non-breaking, together with the repository move:**
C1, C3 (+ flip `test_from1`), C4, C5 (+ nested-cover tests, both orders),
R1 (+ multibyte tests), R3 docs, A1, A2, A4, A5, T1 for the touched
functions plus `cover`, `first_day`/`last_day`, `Display`, negative offsets.
Gate: fmt, clippy `-D warnings`, tests, `cargo publish --dry-run`.

**Phase 2 — 0.2.0, breaking, after the decisions in §4:**
D1 (canonical strings, HalfMonth parser, round-trip property test), D2 (one
century rule, explicit reference year), D3 (one tokenizer), D4 (real error
semantics, dead variant removed), C2 (`try_*` constructors or carry), R2
(fallible calendar functions), D6 (`Copy`), D7 (renames), D5 (optional).

Nothing is published before Dan has read this document.
