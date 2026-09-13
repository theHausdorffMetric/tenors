//! A tour of the core API.

use tenors::*;

fn main() -> Result<(), TenorsError> {
    println!("Tenors tour\n");

    let test_vector: Vec<Tenor> = vec![
        Tenor::new(TenorType::Year, 10),
        Tenor::new(TenorType::HalfYear, 23),
        Tenor::new(TenorType::Quarter, 73),
        Tenor::new(TenorType::Month, 123),
        Tenor::new(TenorType::HalfMonth, 222),
        Tenor::new(TenorType::HalfMonth, 223),
        Tenor::new(TenorType::HalfMonth, 264),
    ];

    println!("Our test vector consists of:");
    for t in &test_vector {
        println!("  {t:?} displays as {t}");
    }

    println!("\nAdding and subtracting offsets");
    for t in &test_vector {
        println!("  {t} + 1 = {}, {t} - 11 = {}", *t + 1, *t - 11);
    }

    println!("\nAdding a TenorDuration of every type");
    for t in &test_vector {
        for ty in TenorType::iterator() {
            match *t + TenorDuration::new(ty) {
                Ok(x) => println!("  {t} + 1 {ty:?} = {x}"),
                Err(e) => println!("  {t} + 1 {ty:?}: {e}"),
            }
        }
    }

    println!("\nPartitioning");
    for t in &test_vector {
        for ty in TenorType::iterator() {
            match t.partition(ty) {
                Some(parts) => println!(
                    "  {t} into {ty:?}: [{}]",
                    parts
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                None => println!("  {t} cannot be partitioned into {ty:?}"),
            }
        }
    }

    println!("\nCovering");
    for t in &test_vector {
        for ty in TenorType::iterator() {
            match t.cover(ty) {
                Some(c) => println!("  {c} covers {t}"),
                None => println!("  no {ty:?} covers {t}"),
            }
        }
    }

    println!("\nCalendar");
    for t in &test_vector {
        println!(
            "  {t}: year {} month0 {:?} quarter0 {:?}, {} .. {}",
            t.year(),
            t.month0(),
            t.quarter0(),
            t.first_day()
                .map(|d| d.to_string())
                .unwrap_or_else(|| "?".into()),
            t.last_day()
                .map(|d| d.to_string())
                .unwrap_or_else(|| "?".into()),
        );
    }

    println!("\nParsing");
    for s in [
        "F24", "F7", "F2024", "1Q24", "24Q1", "2H2025", "Cal24", "2024", "2HF24", "DEC 32",
    ] {
        let parsed = if s.contains(' ') {
            cme_tenor_parser(s)
        } else {
            s.parse::<Tenor>()
        };
        match parsed {
            Ok(t) => println!("  {s:>7} -> {t:?} -> {t}"),
            Err(e) => println!("  {s:>7} -> {e}"),
        }
    }
    println!(
        "  F24 relative to 1950: {}",
        Tenor::parse_with_reference("F24", 1950)?
    );

    Ok(())
}
