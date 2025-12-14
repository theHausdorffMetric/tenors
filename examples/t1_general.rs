use anyhow::Result;
use tenors::*;

fn main() -> Result<()> {

    println!("\nTenors Testing:");

    // test  vector

    let test_vector: Vec<Tenor> = vec![
        Tenor::new(TenorType::Year, 10),
        Tenor::new(TenorType::HalfYear, 23),
        Tenor::new(TenorType::Quarter, 73),
        Tenor::new(TenorType::Month, 123),
        Tenor::new(TenorType::HalfMonth, 222),
        Tenor::new(TenorType::HalfMonth, 223),
        Tenor::new(TenorType::HalfMonth, 264),
    ];

    println!("\nOur test vector consists of:");
    for i in &test_vector {
        println!("{i:?} display: {i}",);
    }

    println!("\nTesting direct addition of isize to Tenor");
    for i in &test_vector {
        let tst = i + 1;
        println!("{i:?} resp. {i} +  1 = {:?} {}", tst, tst);
    }
    for i in &test_vector {
        let tst = i - 1;
        println!("{i:?} resp. {i} -  1 = {:?} {}", tst, tst);
    }
    for i in &test_vector {
        let tst = i + 11;
        println!("{i:?} resp. {i} +  11 = {:?} {}", tst, tst);
    }
    for i in &test_vector {
        let tst = i - 11;
        println!("{i:?} resp. {i} -  11 = {:?} {}", tst, tst);
    }

    println!("\nTesting addition of TenorDuration to Tenor");
    for i in &test_vector {
        for j in TenorType::iterator() {
            match i + TenorDuration::new(j) {
                Ok(x) => println!("{i} + TenorDuration{j:?} = {x}"),
                Err(x) => println!("{i} + TenorDuration{j:?} = {x:?}"),
            };
        }
    }

    println!("\nTesting partitioning of Tenor");
    for i in &test_vector {
        println!("Partitioning {i}");
        for j in TenorType::iterator() {
            match i.partition(j) {
                Some(x) => println!(
                    "{i}: [{}]",
                    x.iter()
                        .map(|z| z.to_string())
                        .collect::<Vec<String>>()
                        .join(", ")
                ),
                None => println!("Cannot partition {:?} into {j:?}", i.get_tenor_type()),
            };
        }
    }

    println!("\nTesting cover function.");
    for i in &test_vector {
        println!("Covering {i}");
        for j in TenorType::iterator() {
            match i.cover(j) {
                Some(x) => println!("{x} covers {i}",),
                None => println!("No {j:?} covering {:?}", i.get_tenor_type()),
            };
        }
    }

    println!("\nTesting year() and month0() functions.");
    for i in &test_vector {
        println!(
            "{i:?} display: {i} with year {:?} and month0 {:?}",
            i.year(),
            i.month0(),
        );
    }

    println!("\nTesting first and last functions.");
    for i in &test_vector {
        println!(
            "{i:?} display: {i} with first {} and last {}",
            i.first_day(),
            i.last_day(),
        );
    }

    println!("\nTesting futures code stuff.");
    println!("F24: {}", Tenor::month_from_futures_code("F24").unwrap());
    println!("F24: {:?}", Tenor::month_from_futures_code("F24"));

    Ok(())
}
