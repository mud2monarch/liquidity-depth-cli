use clap::Parser;
use std::{
    time::Instant,
    ops::{Div, Mul},
};
use num_bigint::BigUint;
use alloy_primitives::{utils::format_units, U256};

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

/// A function to check if slippage is within a given tolerance.
/// Implemented entirely with U256s.
/// 
/// Terminology - PREFIXES:
/// - spot_ = the spot price
/// - counter_ = the counterfactual price, i.e., the simulated output
/// - tol_ = the tolerance, i.e., the slippage tolerance
/// Terminology - SUFFIXES:
/// - _num = the numerator of a number
/// - _den = the denominator of a number
/// 
/// Slippage is therefore (counter_price - spot_price) / spot_price.
/// 
/// Args:
/// - spot_num: The numerator of the spot price
/// - spot_den: The denominator of the spot price
/// - counter_num: The numerator of the counterfactual price
/// - counter_den: The denominator of the counterfactual price
/// - tol_num: The numerator of the tolerance
/// - tol_den: The denominator of the tolerance
/// 
/// Returns:
/// - True if the slippage is within the tolerance, false otherwise
/// 
/// TODO: prevent overflows
fn within_slippage_ints(
    spot_num: U256,
    spot_den: U256,
    counter_num: U256,
    counter_den: U256,
    tol_num: U256,
    tol_den: U256,
) -> bool {
    counter_num * spot_den * tol_den <= spot_num * counter_den * tol_num
}

/// Function to calculate the output amount for a given slippage tolerance.
/// 
/// Args:
/// - slippage: The slippage tolerance, as a decimal (e.g., 2% slippage = 0.02)
/// - state: a Tycho-Simulation "state," i.e., an object that implements `ProtocolSim`.
/// Typically this will come from a BlockUpdate.states.
/// 
/// Returns:
/// - The output amount for the given slippage tolerance
fn calculate_output_for_slippage_tolerance<S: ProtocolSim>(
    slippage: f64,
    state: &S,
    token_in: Token,
    token_out: Token,
) -> U256 {
    let mut left: U256 = U256::from(0);
    let mut right: U256 = U256::from(1_000_000_000u64);
    let mut try_in: U256 = (left + right) / U256::from(2);

    // We pick an arbitrary scalar. TODO: will need to test this for edge cases in token amounts,
    // but for now let's get this working for ETH/USDC.
    let scale: u128 = 1_000_000_000; 

    // Decompose our tolerance into two ints
    let tol_num: U256 = U256::from((slippage * scale as f64).round() as u128);
    let tol_den: U256 = U256::from(scale);

    // Get the spot price
    let spot_price_float: f64 = state.spot_price(&token_in, &token_out).expect("Failed to get spot price");
    let spot_num: U256 = U256::from((p * scale as f64).round() as u128);
    let spot_den: U256 = U256::from(scale);

    
    let mut good_slippage: bool = false;
    while(!good_slippage) {
        // TODO: LEaving off here for today. I'm evaluating valid but that's skipping a step. I need to move up the left and right pointers.
        let try_out: U256 = U256::from(state
            .get_amount_out(try_in, token_in, token_out)
            .unwrap()
            .amount);
        let valid: bool = within_slippage_ints(
        spot_num,
        spot_den,
        try_out,
        try_in,
        tol_num,
        tol_den,
        );
        
        if valid {
            good_slippage = true;
        } else {
            try_in * U256::from(2);
        }
    }

    try_in

}

/// NOTES
/// If I'm trying to get the number of USDC out for 1 ETH:
/// BASE TOKEN = ETH
/// QUOTE TOKEN = USDC
/// AMOUNT IN = 1 ETH
/// AMOUNT OUT = 2700 USDC