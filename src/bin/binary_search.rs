use clap::Parser;
use std::time::Instant;
use num_bigint::BigUint;

#[derive(Parser)]
struct Cli {
    /// The target number
    #[clap(long, short, default_value = "1000.0")]
    pub target: f64,
}

fn main () {
    let start = Instant::now();

    let cli = Cli::parse();
    let target: f64 = cli.target;
    let mut left: f64 = 0.0;
    let mut right: f64 = 10000000000000000.0;
    let mut mid: f64 = right / 2.0;
    let mut guesses: usize = 0;

    if target < left || target > right {
        println!("Target {} is outside of bounds [{}, {}]", target, left, right);
        return;
    }

    while (mid-target).abs() > 1e-3 {
        if mid < target {
            left = mid;
        } else {
            right = mid;
        }
        mid = (left + right) / 2.0;
        guesses += 1;
    }
    // assert_eq!(mid, target);
    println!("Found target: {}", mid);
    let duration = start.elapsed();
    println!("Time taken: {:?} with {} guesses", duration, guesses);
}

/// Get the target output amount at n depth based on a given spot price.
/// 
/// Args:
/// - depth: The target depth of the output, as a decimal (e.g., 2% depth = 0.02)
/// - spot_price: The spot price of the token pair
/// 
/// Returns:
/// - The target output amount at n depth based on the given spot price.
fn get_n_deep_output(
    depth: f64,
    spot_price: BigUint,
) -> f64 {
    (spot_price - (spot_price * depth)).abs()
}