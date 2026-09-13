//! Forward-curve interpolation from cover constraints.

use tenors::TenorType;
use tenors::interpolation::{TenorPoint, interpolate_from_covers, validate_curve};

fn show(title: &str, curve: &mut Vec<TenorPoint>, target: TenorType) {
    println!("\n{title}\n{}", "-".repeat(title.len()));
    println!("before:");
    for p in curve.iter() {
        println!("  {} = {:.2}", p.tenor, p.value);
    }
    match interpolate_from_covers(curve, target, 1e-9, TenorPoint::new) {
        Ok(added) => {
            println!(
                "interpolated {} tenor(s): {}",
                added.len(),
                added
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            curve.sort_by(|a, b| a.tenor.cmp_by_granularity(&b.tenor));
            println!("after:");
            for p in curve.iter() {
                println!("  {} = {:.2}", p.tenor, p.value);
            }
            println!(
                "validate: {:?}",
                validate_curve(curve, target, 1e-9).map_err(|e| e.to_string())
            );
        }
        Err(e) => println!("error: {e}"),
    }
}

fn main() {
    let t = |s: &str| s.parse().unwrap();

    show(
        "Example 1: H25 from the Q1 constraint",
        &mut vec![
            TenorPoint::new(t("F25"), 100.0),
            TenorPoint::new(t("G25"), 105.0),
            TenorPoint::new(t("1Q25"), 103.33),
        ],
        TenorType::Month,
    );
    show(
        "Example 2: G25 and H25 from the Q1 constraint",
        &mut vec![
            TenorPoint::new(t("F25"), 90.0),
            TenorPoint::new(t("1Q25"), 100.0),
        ],
        TenorType::Month,
    );
    show(
        "Example 3: Q3 and Q4 from the calendar year",
        &mut vec![
            TenorPoint::new(t("1Q25"), 90.0),
            TenorPoint::new(t("2Q25"), 95.0),
            TenorPoint::new(t("Cal25"), 100.0),
        ],
        TenorType::Quarter,
    );
    show(
        "Example 4: nested covers, given coarse-first — order no longer matters",
        &mut vec![
            TenorPoint::new(t("Cal25"), 106.25),
            TenorPoint::new(t("2H25"), 110.0),
            TenorPoint::new(t("2Q25"), 104.0),
            TenorPoint::new(t("F25"), 100.0),
            TenorPoint::new(t("G25"), 101.0),
            TenorPoint::new(t("H25"), 102.0),
        ],
        TenorType::Month,
    );
    show(
        "Example 5: contradictory covers are an error, not a silent curve",
        &mut vec![
            TenorPoint::new(t("1Q25"), 100.0),
            TenorPoint::new(t("2Q25"), 100.0),
            TenorPoint::new(t("3Q25"), 100.0),
            TenorPoint::new(t("4Q25"), 100.0),
            TenorPoint::new(t("Cal25"), 120.0),
        ],
        TenorType::Month,
    );
}
