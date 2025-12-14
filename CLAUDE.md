# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust crate for working with financial tenors (time periods used in financial contracts). The library provides types and operations for manipulating various tenor types including Years, Half-Years, Quarters, Months, and Half-Months.

## Core Architecture

### Main Components

1. **Tenor** - Primary struct representing a specific point in time with a type and offset from epoch (1970)
2. **TenorType** - Enum defining granularity levels (HalfMonth, Month, Quarter, HalfYear, Year)
3. **TenorDuration** - Represents a duration that can be added to Tenors
4. **Futures Code Parsing** - Utilities for parsing financial futures contract codes

### Key Design Principles

- Tenors are encoded as offsets from a 1970 epoch
- Different tenor types have different granularities (HalfMonth = 1, Year = 24)
- Partial ordering only exists between tenors of the same type
- Operations preserve type safety through Result types for incompatible operations

## Development Commands

### Build
```bash
cargo build           # Debug build
cargo build --release # Release build
```

### Test
```bash
cargo test            # Run all tests
cargo test -- --nocapture  # Show test output
```

### Lint & Format
```bash
cargo clippy          # Run linter
cargo fmt             # Format code
cargo fmt --check     # Check formatting without changes
```

### Documentation
```bash
cargo doc             # Generate documentation
cargo doc --open      # Generate and open docs in browser
```

### Examples
```bash
cargo run --example t1_general  # Run the general examples
```

## Testing Guidelines

The crate includes comprehensive unit tests in `src/lib.rs`. When adding new functionality:
- Add tests for edge cases (boundary months, year transitions)
- Test error conditions for incompatible tenor operations
- Verify formatting/display output matches expected financial conventions

## Error Handling

The crate uses `thiserror` for error definitions. Main error types:
- `IncompatibleTenors` - Operations between incompatible tenor types
- `UnparsableTenorString` - Invalid futures code or tenor string parsing

## Dependencies

Core dependencies:
- `chrono` - Date/time operations
- `serde` - Serialization support
- `thiserror` - Error handling
- `anyhow` - Error handling in examples
- `regex` - String parsing
- `log`/`env_logger` - Logging support