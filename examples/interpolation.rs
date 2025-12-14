//! Example demonstrating forward curve interpolation using cover constraints.

use anyhow::Result;
use tenors::interpolation::{interpolate_from_covers, validate_curve, TenorPoint};
use tenors::TenorType;

fn main() -> Result<()> {

    println!("=== Forward Curve Interpolation Example ===\n");

    // Example 1: Interpolate missing month from quarter constraint
    println!("Example 1: Interpolate H25 from Q1 constraint");
    println!("---------------------------------------------");

    let mut curve = vec![
        TenorPoint::new("F25".parse()?, 100.0),
        TenorPoint::new("G25".parse()?, 105.0),
        TenorPoint::new("1Q25".parse()?, 103.33),
    ];

    println!("Before interpolation:");
    for p in &curve {
        println!("  {} = {:.2}", p.tenor, p.value);
    }

    let interpolated =
        interpolate_from_covers(&mut curve, TenorType::Month, TenorPoint::new)?;

    println!("\nInterpolated tenors: {:?}", interpolated);
    println!("\nAfter interpolation:");
    curve.sort_by(|a, b| a.tenor.cmp_by_granularity(&b.tenor));
    for p in &curve {
        println!("  {} = {:.2}", p.tenor, p.value);
    }

    // Example 2: Interpolate multiple missing months
    println!("\n\nExample 2: Interpolate G25 and H25 from Q1 constraint");
    println!("------------------------------------------------------");

    let mut curve2 = vec![
        TenorPoint::new("F25".parse()?, 90.0),
        TenorPoint::new("1Q25".parse()?, 100.0),
    ];

    println!("Before interpolation:");
    for p in &curve2 {
        println!("  {} = {:.2}", p.tenor, p.value);
    }

    let interpolated2 =
        interpolate_from_covers(&mut curve2, TenorType::Month, TenorPoint::new)?;

    println!("\nInterpolated tenors: {:?}", interpolated2);
    println!("\nAfter interpolation:");
    curve2.sort_by(|a, b| a.tenor.cmp_by_granularity(&b.tenor));
    for p in &curve2 {
        println!("  {} = {:.2}", p.tenor, p.value);
    }

    // Example 3: Year to quarters interpolation
    println!("\n\nExample 3: Interpolate Q3 and Q4 from Cal25 constraint");
    println!("-------------------------------------------------------");

    let mut curve3 = vec![
        TenorPoint::new("1Q25".parse()?, 90.0),
        TenorPoint::new("2Q25".parse()?, 95.0),
        TenorPoint::new("Cal25".parse()?, 100.0),
    ];

    println!("Before interpolation:");
    for p in &curve3 {
        println!("  {} = {:.2}", p.tenor, p.value);
    }

    let interpolated3 =
        interpolate_from_covers(&mut curve3, TenorType::Quarter, TenorPoint::new)?;

    println!("\nInterpolated tenors: {:?}", interpolated3);
    println!("\nAfter interpolation:");
    curve3.sort_by(|a, b| a.tenor.cmp_by_granularity(&b.tenor));
    for p in &curve3 {
        println!("  {} = {:.2}", p.tenor, p.value);
    }

    // Example 4: Validate a consistent curve
    println!("\n\nExample 4: Validate curve consistency");
    println!("--------------------------------------");

    let consistent_curve = vec![
        TenorPoint::new("F25".parse()?, 100.0),
        TenorPoint::new("G25".parse()?, 105.0),
        TenorPoint::new("H25".parse()?, 110.0),
        TenorPoint::new("1Q25".parse()?, 105.0), // Average of 100, 105, 110
    ];

    match validate_curve(&consistent_curve, TenorType::Month, 0.01) {
        Ok(()) => println!("Curve is consistent!"),
        Err(e) => println!("Curve validation failed: {}", e),
    }

    // Example 5: Detect an inconsistent curve
    println!("\n\nExample 5: Detect inconsistent curve");
    println!("-------------------------------------");

    let inconsistent_curve = vec![
        TenorPoint::new("F25".parse()?, 100.0),
        TenorPoint::new("G25".parse()?, 105.0),
        TenorPoint::new("H25".parse()?, 110.0),
        TenorPoint::new("1Q25".parse()?, 120.0), // Wrong! Should be 105
    ];

    match validate_curve(&inconsistent_curve, TenorType::Month, 0.01) {
        Ok(()) => println!("Curve is consistent!"),
        Err(e) => println!("Curve validation failed: {}", e),
    }

    // Example 6: Multi-tenor interpolation with Q2, H2, Cal25, Cal26
    println!("\n\nExample 6: Interpolate months from mixed Q2, H2, and yearly covers");
    println!("-------------------------------------------------------------------");

    // Curve with:
    // - 2Q25 = 104 (covers J25, K25, M25)
    // - 2H25 = 110 (covers N25, Q25, U25, V25, X25, Z25)
    // - Cal25 = 106.25 (consistent: (100+101+102 + 3*104 + 6*110)/12 = 106.25)
    // - Cal26 = 118 (covers all 12 months of 2026)
    // Plus a few anchor months
    let mut curve6 = vec![
        // Q1 2025 anchor months
        TenorPoint::new("F25".parse()?, 100.0), // Jan 25
        TenorPoint::new("G25".parse()?, 101.0), // Feb 25
        TenorPoint::new("H25".parse()?, 102.0), // Mar 25
        // Q2 2025 cover (no anchor months - will use constant)
        TenorPoint::new("2Q25".parse()?, 104.0),
        // H2 2025 cover (no anchor months - will use constant)
        TenorPoint::new("2H25".parse()?, 110.0),
        // Full year 2025 cover (consistent with Q1 actuals + Q2 + H2)
        TenorPoint::new("Cal25".parse()?, 106.25),
        // 2026: two anchors + yearly cover
        TenorPoint::new("F26".parse()?, 112.0), // Jan 26
        TenorPoint::new("G26".parse()?, 113.0), // Feb 26
        // Full year 2026 cover
        TenorPoint::new("Cal26".parse()?, 118.0),
    ];

    println!("Before interpolation:");
    curve6.sort_by(|a, b| a.tenor.cmp_by_granularity(&b.tenor));
    for p in &curve6 {
        println!("  {} = {:.2}", p.tenor, p.value);
    }

    let interpolated6 =
        interpolate_from_covers(&mut curve6, TenorType::Month, TenorPoint::new)?;

    println!("\nInterpolated {} tenors", interpolated6.len());
    println!("\nAfter interpolation:");
    curve6.sort_by(|a, b| a.tenor.cmp_by_granularity(&b.tenor));
    for p in &curve6 {
        println!("  {} = {:.2}", p.tenor, p.value);
    }

    // Verify consistency
    println!("\nVerifying curve consistency...");
    match validate_curve(&curve6, TenorType::Month, 0.01) {
        Ok(()) => println!("Curve is consistent!"),
        Err(e) => println!("Curve validation failed: {}", e),
    }

    Ok(())
}
