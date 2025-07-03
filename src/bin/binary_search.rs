use clap::Parser;
use std::{
    time::Instant,
    ops::{Div, Mul},
    fmt::{self, Debug},
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
use tracing::{info, error, warn, debug};

pub struct Slippage {
    pub num: U256,
    pub den: U256,
}

impl Slippage {
    pub fn new(num: U256, den: U256) -> Self {
        Self { num, den }
    }
}

impl std::fmt::Debug for Slippage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Slippage {{ num: {}, den: {} }}", self.num, self.den)
    }
}

#[derive(Debug)]
pub enum SlippageError {
    Overflow,
}

/// A function to calculate the slippage between a counterfactual and spot price.
/// 
/// Args:
/// - counterfactual: The counterfactual price, i.e., the simulated output
/// - spot: The spot price
/// 
/// Returns:
/// - The slippage as a U256, or an error for overflows
pub fn calc_slippage (
    counterfactual: &U256,
    spot: &U256,
) -> Result<Slippage, SlippageError> {
    let slip_num: U256 = counterfactual
        .checked_sub(*spot)
        .ok_or(SlippageError::Overflow)?;

    let slip_den: U256 = *spot;

    let slippage: Slippage = Slippage::new(slip_num, slip_den);

    Ok(slippage)
}

/// A function to check if a given slippage is under a target size, expressed as a decimal.
/// 
/// Args:
/// - slippage: The slippage to check
/// - target_slippage: The target slippage, expressed as a decimal, e.g., 0.02 for 2%
/// 
/// Returns:
/// - True if the slippage is <= the target, false otherwise

pub fn check_slippage_under_target(
    slippage: &Slippage,
    target_slippage: f64,
) -> bool {
    // Set precision to 1,000,000 for this.
    let scale: f64 = 1_000_000.0; 

    // Decompose our precision into two ints
    let targ_num: U256 = U256::from((target_slippage * scale).round() as u128);
    let targ_den: U256 = U256::from(scale);

    &slippage.num * &targ_den <= &slippage.den * &targ_num
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
pub fn check_slippage_vs_target_within_tolerance(
    slippage: &Slippage,
    target_slippage: f64,
    precision: f64,
) -> Result<bool, SlippageError> {
    // Set precision to 1 billion for this.
    let scale: f64 = 1_000_000_000.0; 

    // Decompose our target slippage into two ints  
    let targ_num: U256 = U256::from((target_slippage * scale).round() as u128);
    let targ_den: U256 = U256::from(scale);

    // Decompose our precision into two ints
    let prec_num: U256 = U256::from((precision * scale).round() as u128);
    let prec_den: U256 = U256::from(scale);

    let abs_diff: U256 = if
        slippage.num
        .checked_mul(targ_den).ok_or(SlippageError::Overflow)?
            >
        targ_num
        .checked_mul(slippage.den).ok_or(SlippageError::Overflow)? {
            slippage.num
            .checked_mul(targ_den).ok_or(SlippageError::Overflow)?
                -
            targ_num
            .checked_mul(slippage.den).ok_or(SlippageError::Overflow)?
        } else {
            targ_num
            .checked_mul(slippage.den).ok_or(SlippageError::Overflow)?
                -
            slippage.num
            .checked_mul(targ_den).ok_or(SlippageError::Overflow)?
    };
    
    let lhs: U256 = prec_den.checked_mul(abs_diff).ok_or(SlippageError::Overflow)?;
    let rhs: U256 = prec_num
        .checked_mul(slippage.den).ok_or(SlippageError::Overflow)?
        .checked_mul(targ_den).ok_or(SlippageError::Overflow)?;

    Ok(lhs <= rhs)
}

pub fn main() {
    println!("Hello, world!");
}

// Function to calculate the output amount for a given slippage tolerance.
// 
// Args:
// - slippage: The slippage tolerance, as a decimal (e.g., 2% slippage = 0.02)
// - precision: The precision of the tolerance, i.e., the the range within which we consider the slippage to be exact
// - state: a Tycho-Simulation "state." Typically this will come from a BlockUpdate.states.
// 
// Returns:
// - The output amount for the given slippage tolerance
// pub fn calculate_output_for_slippage_tolerance(
//     target_slippage_float: f64, // TODO: make this a Slippage struct
//     precision: f64,
//     state: &Box<dyn ProtocolSim>,
//     token_in: &Token,
//     token_out: &Token,
// ) -> U256 {
//     let mut left: U256 = U256::from(0);
//     let mut right: U256 = U256::from(1_000_000_000u64);
//     // let mut try_in: U256 = (left + right) / U256::from(2);
//     let mut try_in: U256 = U256::from(10_u64).pow(U256::from(18_u64)); // i.e., 1 ETH

//     // We pick an arbitrary scalar. TODO: will need to test this for edge cases in token amounts,
//     // but for now let's get this working for ETH/USDC.
//     let scale: u128 = 1_000; 

//     // Decompose our target_slippage into two ints
//     let targ_num: U256 = U256::from((target_slippage_float * scale as f64).round() as u128);
//     let targ_den: U256 = U256::from(scale);
//     let target_slippage: Slippage = Slippage::new(targ_num, targ_den);

//     // Decompose our precision into two ints
//     let prec_num: U256 = U256::from((precision * scale as f64).round() as u128);
//     let prec_den: U256 = U256::from(scale);

//     // Get the spot price
//     let spot_price_float: f64 = state.spot_price(&token_in, &token_out).expect("Failed to get spot price");
//     let spot_num: U256 = U256::from((spot_price_float * scale as f64).round() as u128);
//     let spot_den: U256 = U256::from(scale);

//     info!("Initial value assignments --------");
//     info!("left: {:?}", left);
//     info!("right: {:?}", right);
//     info!("try_in: {:?}", try_in);
//     info!("scale: {:?}", scale);
//     info!("targ_num: {:?}", targ_num);
//     info!("targ_den: {:?}", targ_den);
//     info!("prec_num: {:?}", prec_num);
//     info!("spot_price_float: {:?}", spot_price_float);
//     info!("spot_num: {:?}", spot_num);
//     info!("spot_den: {:?}", spot_den);
//     info!("target_slippage_float: {:?}", target_slippage_float);
//     info!("target_slippage: {:?}", target_slippage);
//     info!("precision: {:?}", precision);
//     info!("--------------------------------");
    
//     // First we need to double the amount in until we exceed our target slippage    
//     let mut more_than_slippage_target: bool = false;
//     while !more_than_slippage_target {
//         info!("About to try to get amount out.");
//         info!("try_in: {:?}", try_in);
//         info!("token_in: {:?}", token_in);
//         info!("token_out: {:?}", token_out);

//         let try_out_biguint: BigUint = state.clone()
//                 .get_amount_out(u256_to_biguint(try_in), &token_in, &token_out)
//                 .unwrap()
//                 .amount;

//         info!("try_out_biguint: {:?}", try_out_biguint);

//         let try_out: U256 = biguint_to_u256(&try_out_biguint);

//         let slippage: Slippage = calc_slippage(spot_num, spot_den, try_out, try_in);
        
//         info!("printing slippage ---- \n Try_out: {:?} of type {:?}\n Slippage: {:?}", try_out, std::any::type_name_of_val(&try_out), slippage);

//         let under_target: bool = check_slippage_under_target(
//             &slippage,
//             target_slippage_float,
//             );
        
//         if under_target {
//             left = try_in;
//             try_in = try_in * U256::from(2_u32);
//             info!("hit the under_target block");
//         } else {
//             more_than_slippage_target = true;
//             right = try_in;
//             info!("set more than slippage target to true");
//         };
//     }

//     // We now know that the amount in is greater than our target slippage threshold.
//     try_in = (left + right) / U256::from(2);

//     let mut slippage_within_tolerance: bool = false;
//     // Now we binary search for the exact amount in.
//     while !slippage_within_tolerance {
//         let try_out: U256 = biguint_to_u256(
//             &state
//             .get_amount_out(u256_to_biguint(try_in), &token_in, &token_out)
//             .unwrap()
//             .amount)
//         ;

//         let slippage: Slippage = calc_slippage(spot_num, spot_den, try_out, try_in);

//         let valid: bool = check_slippage_vs_target_within_tolerance(
//             &slippage,
//             target_slippage_float,
//             precision);

//         if valid {
//             slippage_within_tolerance = true;
//         } else {
//             if &slippage.num * &target_slippage.den < &target_slippage.num * &slippage.den {
//                 left = try_in;
//                 try_in = (left + right) / U256::from(2);
//             } else if &slippage.num * &target_slippage.den > &target_slippage.num * &slippage.den {
//                 right = try_in;
//                 try_in = (left + right) / U256::from(2);
//             } else {
//                 panic!("This state should be unreachable.")
//             }
//         }
//     }

//     try_in
// }

// NOTES
// If I'm trying to get the number of USDC out for 1 ETH:
// BASE TOKEN = ETH
// QUOTE TOKEN = USDC
// AMOUNT IN = 1 ETH
// AMOUNT OUT = 2700 USDC