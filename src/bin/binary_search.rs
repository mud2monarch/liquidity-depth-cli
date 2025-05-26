use clap::Parser;
use std::{
    time::Instant,
    ops::{Div, Mul},
};
use num_bigint::BigUint;
use alloy_primitives::{utils::format_units, U256};
use tycho_simulation::{
    models::Token,
    protocol::{
        state::ProtocolSim,
    },
    evm::protocol::u256_num::{u256_to_biguint, biguint_to_u256},
};
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

// TOOD: Implement Ordering for Slippage
pub struct Slippage {
    pub num: U256,
    pub den: U256,
}

impl Slippage {
    pub fn new(num: U256, den: U256) -> Self {
        Self { num, den }
    }
}
/// A function to calculate the slippage as a numerator and denominator.
/// 
/// Slippage is the difference between the counterfactual and spot price, divided by the spot price.
/// 
/// Terminology - PREFIXES:
/// - spot_ = the spot price
/// - counter_ = the counterfactual price, i.e., the simulated output
/// Terminology - SUFFIXES:
/// - _num = the numerator of a number
/// - _den = the denominator of a number
/// 
///              counter_num    spot_num
///              -----------  – --------
///  slip_num    counter_den    spot_den
///  -------- =  -----------------------
///  slip_dom             spot_num
///                       --------
///                       spot_den
/// 
/// I am atrocious at algebra, but here's the math: https://drive.google.com/file/d/1h61ZLxg-tf_fl5oZczV_OWMhecbt7fap/view?usp=sharing
///
///  Args:
/// - spot_num: The numerator of the spot price
/// - spot_den: The denominator of the spot price
/// - counter_num: The numerator of the counterfactual price
/// - counter_den: The denominator of the counterfactual price
/// 
/// Returns:
/// - The slippage as (slippage_numerator, slippage_denominator)
fn calc_slippage(
    spot_num: U256,
    spot_den: U256,
    counter_num: U256,
    counter_den: U256,
) -> Slippage {
    let slip_num: U256 = counter_num * spot_den - spot_num * counter_den;
    let slip_den: U256 = counter_den * spot_num;
    Slippage::new(slip_num, slip_den)
}

/// A function to check if a given Slippage is under a target size, expressed as a decimal.
/// 
/// This is equivalent to: slip_num    targ_num
///                        -------- <= --------
///                        slip_den    targ_den
///  Args:
/// - slippage: The slippage to check
/// - target_slippage: The target slippage, expressed as a decimal, e.g., 0.02 for 2%
/// 
/// Returns:
/// - True if the slippage is <= the target, false otherwise

fn check_slippage_under_target(
    slippage: &Slippage,
    target_slippage: f64,
) -> bool {
    // Set precision to 1000 for this.
    let scale: u128 = 1_000_000; 

    // Decompose our precision into two ints
    let targ_num: U256 = U256::from((target_slippage * scale as f64).round() as u128);
    let targ_den: U256 = U256::from(scale);

    slippage.num * targ_den <= slippage.den * targ_num
}

/// A function to check if the slippage is within a given tolerance of the target slippage.
/// 
/// Args:
/// - slippage: The slippage to check
/// - target_slippage: The target slippage, expressed as a decimal, e.g., 0.02 for 2%
/// - precision: The precision of the tolerance, expressed as a decimal, e.g., 0.0001 for 0.01%
/// 
/// slippage.num   targ_num    prec_num
/// ------------ - -------- <= --------
/// slippage.den   targ_den    prec_den
///
/// slippage.num * targ_den - targ_num * slippage.den     prec_num
/// ------------------------------------------------- <=  --------
///              slippage.den * targ_den                  prec_den
/// 
/// prec_den * (slippage.num * targ_den - targ_num * slippage.den) <= prec_num * slippage.den * targ_den
/// 
/// But here I need the absolute value of the difference, so I call the difference "abs_diff" and ensure it's positive
/// with an if/else statement.
/// 
/// prec_den * |abs_diff| <= prec_num * slippage.den * targ_den
/// 
/// Returns: true if the slippage is within { tolerance } of the target slippage, false otherwise
fn check_slippage_vs_target_within_tolerance(
    slippage: &Slippage,
    target_slippage: f64,
    precision: f64,
) -> bool {
    // Set precision to 1 billion for this.
    let scale: u128 = 1_000_000_000; 

    // Decompose our target slippage into two ints  
    let targ_num: U256 = U256::from((target_slippage * scale as f64).round() as u128);
    let targ_den: U256 = U256::from(scale);

    // Decompose our precision into two ints
    let prec_num: U256 = U256::from((precision * scale as f64).round() as u128);
    let prec_den: U256 = U256::from(scale);

    let abs_diff: U256 = if slippage.num * targ_den > targ_num * slippage.den {
        slippage.num * targ_den - targ_num * slippage.den
    } else {
        targ_num * slippage.den - slippage.num * targ_den
    };
    
    prec_den * abs_diff <= prec_num * slippage.den * targ_den
}

// /// A function to check if slippage is within a given tolerance.
// /// Implemented entirely with U256s.
// /// 
// /// Terminology - PREFIXES:
// /// - spot_ = the spot price
// /// - counter_ = the counterfactual price, i.e., the simulated output
// /// - slip_ = the slippage, i.e., the difference between the counterfactual and spot price
// /// Terminology - SUFFIXES:
// /// - _num = the numerator of a number
// /// - _den = the denominator of a number
// /// 
// /// Slippage is therefore (counter_price - spot_price) / spot_price.
// /// 
// /// Args:
// /// - spot_num: The numerator of the spot price
// /// - spot_den: The denominator of the spot price
// /// - counter_num: The numerator of the counterfactual price
// /// - counter_den: The denominator of the counterfactual price
// /// - slip_num: The numerator of the slippage threshold
// /// - slip_den: The denominator of the slippage threshold
// ///
// /// This is equivalent to: counter_num    spot_num   slip_num
// ///                        ----------- <= -------- * --------
// ///                        counter_den    spot_den   slip_den
// /// 
// /// Returns:
// /// - True if the slippage is within the tolerance, false otherwise
// /// 
// /// TODO: prevent overflows
// fn within_slippage(
//     spot_num: U256,
//     spot_den: U256,
//     counter_num: U256,
//     counter_den: U256,
//     slip_num: U256,
//     slip_den: U256,
// ) -> bool {
//     counter_num * spot_den * slip_den <= counter_den * spot_num * slip_num
// }

// /// A function to check if the counterfactual price and spot price are separated exactly by the slippage threshold.
// /// Implemented entirely with U256s.
// /// 
// /// Terminology - PREFIXES:
// /// - spot_ = the spot price
// /// - counter_ = the counterfactual price, i.e., the simulated output
// /// - slip_ = the slippage, i.e., the difference between the counterfactual and spot price
// /// - prec_ = the precision of the tolerance, i.e., the the range within which we consider the slippage to be exact
// /// Terminology - SUFFIXES:
// /// - _num = the numerator of a number
// /// - _den = the denominator of a number
// /// 
// /// Slippage is therefore (counter_price - spot_price) / spot_price.
// /// 
// /// Args:
// /// - spot_num: The numerator of the spot price
// /// - spot_den: The denominator of the spot price
// /// - counter_num: The numerator of the counterfactual price
// /// - counter_den: The denominator of the counterfactual price
// /// - slip_num: The numerator of the slippage threshold
// /// - slip_den: The denominator of the slippage threshold
// ///
// /// This is equivalent to: counter_num    spot_num * slip_num
// ///                        ----------- <= --------   --------
// ///                        counter_den    spot_den   slip_den
// /// 
// /// Returns:
// /// - True if the slippage is within the tolerance, false otherwise
// /// 
// /// TODO: prevent overflows
// fn check_exact_slippage_within_tolerance (
//     spot_num: U256,
//     spot_den: U256,
//     counter_num: U256,
//     counter_den: U256,
//     slip_num: U256,
//     slip_den: U256,
//     prec_num: U256,
//     prec_den: U256,
// ) -> bool {
//     // Split our formula into two sides for readability
//     let lhs: U256 = counter_num * spot_den * slip_den;
//     let rhs: U256 = counter_den * spot_num * slip_num;
    
//     // Calculate the difference, which must be positive
//     let diff: U256 = if lhs > rhs { lhs - rhs } else { rhs - lhs };

//     // 
//     // diff / rhs ≤ ε   ->   diff * eps_den ≤ rhs * eps_num
//     diff * prec_den <= rhs * prec_num
// }

/// Function to calculate the output amount for a given slippage tolerance.
/// 
/// Args:
/// - slippage: The slippage tolerance, as a decimal (e.g., 2% slippage = 0.02)
/// - precision: The precision of the tolerance, i.e., the the range within which we consider the slippage to be exact
/// - state: a Tycho-Simulation "state," i.e., an object that implements `ProtocolSim`.
/// Typically this will come from a BlockUpdate.states.
/// 
/// Returns:
/// - The output amount for the given slippage tolerance
fn calculate_output_for_slippage_tolerance<S: ProtocolSim>(
    target_slippage_float: f64, // TODO: make this a Slippage struct
    precision: f64,
    state: &S,
    token_in: Token,
    token_out: Token,
) -> U256 {
    let mut left: U256 = U256::from(0);
    let mut right: U256 = U256::from(1_000_000_000u64);
    let mut try_in: U256 = (left + right) / U256::from(2);

    // We pick an arbitrary scalar. TODO: will need to test this for edge cases in token amounts,
    // but for now let's get this working for ETH/USDC.
    let scale: u128 = 1_000; 

    // Decompose our target_slippage into two ints
    let targ_num: U256 = U256::from((target_slippage_float * scale as f64).round() as u128);
    let targ_den: U256 = U256::from(scale);
    let target_slippage: Slippage = Slippage::new(targ_num, targ_den);

    // Decompose our precision into two ints
    let prec_num: U256 = U256::from((precision * scale as f64).round() as u128);
    let prec_den: U256 = U256::from(scale);

    // Get the spot price
    let spot_price_float: f64 = state.spot_price(&token_in, &token_out).expect("Failed to get spot price");
    let spot_num: U256 = U256::from((spot_price_float * scale as f64).round() as u128);
    let spot_den: U256 = U256::from(scale);

    // First we need to double the amount in until we exceed our target slippage    
    let mut more_than_slippage_target: bool = false;
    while !more_than_slippage_target {
        let try_out: U256 = biguint_to_u256(
                &state
                .get_amount_out(u256_to_biguint(try_in), &token_in, &token_out)
                .unwrap()
                .amount)
            ;
        
        let slippage: Slippage = calc_slippage(spot_num, spot_den, try_out, try_in);
        
        let under_target: bool = check_slippage_under_target(
            &slippage,
            target_slippage_float,
            );
        
        if under_target {
            left = try_in;
            try_in = try_in * U256::from(2_u32);
        } else {
            more_than_slippage_target = true;
            right = try_in;
        };
    }

    // We now know that the amount in is greater than our target slippage threshold.
    try_in = (left + right) / U256::from(2);

    let mut slippage_within_tolerance: bool = false;
    // Now we binary search for the exact amount in.
    while !slippage_within_tolerance {
        let try_out: U256 = biguint_to_u256(
            &state
            .get_amount_out(u256_to_biguint(try_in), &token_in, &token_out)
            .unwrap()
            .amount)
        ;

        let slippage: Slippage = calc_slippage(spot_num, spot_den, try_out, try_in);

        let valid: bool = check_slippage_vs_target_within_tolerance(
            &slippage,
            target_slippage_float,
            precision);

        if valid {
            slippage_within_tolerance = true;
        } else {
            if &slippage.num * &target_slippage.den < &target_slippage.num * &slippage.den {
                left = try_in;
                try_in = (left + right) / U256::from(2);
            } else if &slippage.num * &target_slippage.den > &target_slippage.num * &slippage.den {
                right = try_in;
                try_in = (left + right) / U256::from(2);
            } else {
                panic!("This state should be unreachable.")
            }
        }
    }

    try_in
}

// NOTES
// If I'm trying to get the number of USDC out for 1 ETH:
// BASE TOKEN = ETH
// QUOTE TOKEN = USDC
// AMOUNT IN = 1 ETH
// AMOUNT OUT = 2700 USDC