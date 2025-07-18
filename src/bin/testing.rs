use std::{
    collections::HashMap,
    env,
};
use liquidity_depth_cli::binary_search::*;
use num_bigint::BigUint;
use tycho_common::{
    models::Chain,
    Bytes,
};
use num_bigint::ToBigUint;
use tycho_simulation::{
    protocol::{
        state::ProtocolSim,
        models::BlockUpdate,
    },
    evm::{
        engine_db::tycho_db::PreCachedDB,
        protocol::{
            ekubo::state::EkuboState, 
            filters::{balancer_pool_filter, curve_pool_filter, uniswap_v4_pool_with_hook_filter},
            u256_num::u256_to_biguint,
            uniswap_v2::state::UniswapV2State,
            uniswap_v3::state::UniswapV3State,
            uniswap_v4::state::UniswapV4State,
            vm::state::EVMPoolState,
        },
        stream::ProtocolStreamBuilder,
    },
    models::Token,
    tycho_client::feed::component_tracker::ComponentFilter,
    utils::load_all_tokens
};
use futures::StreamExt;
use tracing::{info, error, warn, debug};
use tracing_subscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    // ── env / CLI boilerplate ──────────────────────────────────────────────────
    let chain = Chain::Unichain;
    let tycho_url = env::var("TYCHO_URL")
        .unwrap_or_else(|_| String::from("tycho-unichain-beta.propellerheads.xyz"));
    let tycho_api_key =
        env::var("TYCHO_API_KEY").unwrap_or_else(|_| String::from("sampletoken"));

    // load full token list once
    let tokens = load_all_tokens(
        &tycho_url,
        false,
        Some(&tycho_api_key),
        chain,
        None,
        None,
    )
    .await;

    let usdc = Token::new(
        "0x078D782b760474a361dDA0AF3839290b0EF57AD6",
        6,
        "USDC",
        10_000.to_biguint().unwrap()
    );
    let native_eth = Token::new(
        "0x0000000000000000000000000000000000000000",
        18,
        "ETH",
        10_000.to_biguint().unwrap()
    );
    let mut test_pair = vec![usdc.clone(), native_eth.clone()];
    test_pair.sort_unstable_by_key(|t: &Token| t.address.clone());

    // ── build exactly the same ProtocolStream as in main.rs ───────────────────
    let tvl_filter = ComponentFilter::with_tvl_range(500.0, 500.0);
    let mut stream = register_exchanges(
        ProtocolStreamBuilder::new(&tycho_url, chain),
        &chain,
        tvl_filter,
    )
    .auth_key(Some(tycho_api_key.clone()))
    .skip_state_decode_failures(true)
    .set_tokens(tokens.clone())
    .await
    .build()
    .await
    .expect("failed to build protocol stream");

    println!("🛰  waiting for first block …");
    
    println!("test tokens: {:?}", test_pair);
    let mut blocks_seen = 0;
    let mut tracked_pairs = HashMap::new();
    let mut tracked_states = HashMap::new();

    while let Some(msg) = stream.next().await {
        let block = msg?;
        // update tracked pairs
        for (id, pool) in block.new_pairs.iter() {
            tracked_pairs.insert(id.clone(), pool.tokens.clone());
        }
        for (id, pool) in block.removed_pairs.iter() {
            tracked_pairs.remove(id);
        }

        for (id, state) in block.states.iter() {
            tracked_states.insert(id.clone(), state.clone());
        }

        blocks_seen += 1;

        println!("Block #{}", block.block_number);
        println!("   → {} states", block.states.len());
        println!("   → {} new pairs", block.new_pairs.len());
        println!("   → {} removed pairs", block.removed_pairs.len());
        
        for (id, tokens) in tracked_pairs.iter() {
            if tokens == &test_pair {
                let state: &Box<dyn ProtocolSim> = tracked_states.get(id)
                    .unwrap();
                let out = state.clone()
                    .get_amount_out(
                        u256_to_biguint(native_eth.one()),
                        &native_eth,
                        &usdc)
                        .expect("failed to get amount out")
                        .amount;
                println!("✅ 1 ETH = {} USDC", out);

                // TODO: start testing here
                // We need to get the matching state from tracked_states
                
                let slippage: f64 = 0.02;
                let precision: f64 = 0.0001;
                let output_for_two_percent = calculate_output_for_slippage_tolerance(
                    slippage,
                    precision,
                    state,
                    &native_eth,
                    &usdc);

                println!("Output for 2% slippage: {:?}", output_for_two_percent);
            } else {
                println!("🔴 skipping pair {} - {}", tokens[0].symbol, tokens[1].symbol);
                // println!("This is Token {:?}", tokens);
            }
        };

        if blocks_seen >= 5 {
            println!("Seen {} blocks", blocks_seen);

            break;
        }
    };

    // // ── consume a single block update ─────────────────────────────────────────
    // while let Some(msg) = stream.next().await {
    //     let block = msg?; // Result<BlockUpdate>
    //     println!("📦  got block {}", block.block_number);

    //     // iterate over protocol components in this block
    //     for comp in &block.states {
    //         // look for a UniV3 pool with both tokens
    //         if comp.protocol_system != "uniswap_v3" {
    //             continue;
    //         }
    //         if !comp.tokens.contains(&weth_addr.to_lowercase())
    //             || !comp.tokens.contains(&usdc_addr.to_lowercase())
    //         {
    //             continue;
    //         }

    //         println!("🎯  found pool {}", comp.id);

    //         // instantiate pool state
    //         let state = UniswapV3State::new(
    //             comp,
    //             &tycho_url,
    //             Some(&tycho_api_key),
    //             &tokens,
    //         )
    //         .await?;

    //         // spot price
    //         let spot = state.spot_price(native_eth, usdc)?;
    //         println!("🟢 spot_price 1 WETH → {spot:.6} USDC");

    //         // amount-out for 1 WETH
    //         let one_weth = BigUint::from(1_000_000_000_000_000_000u128);
    //         let out = state.get_amount_out(one_weth.clone(), native_eth, usdc)?;
    //         let out_f64 = out.amount.to_f64().unwrap() / 1e6; // USDC has 6 dec
    //         println!("💸 get_amount_out: 1 WETH → {out_f64:.2} USDC (gas {})", out.gas);

    //         return Ok(()); // done
    //     }
    // }

    Ok(())
}

fn register_exchanges(
    mut builder: ProtocolStreamBuilder,
    chain: &Chain,
    tvl_filter: ComponentFilter,
) -> ProtocolStreamBuilder {
    match chain {
        Chain::Ethereum => {
            builder = builder
                .exchange::<UniswapV2State>("uniswap_v2", tvl_filter.clone(), None)
                .exchange::<UniswapV3State>("uniswap_v3", tvl_filter.clone(), None)
                .exchange::<EVMPoolState<PreCachedDB>>(
                    "vm:balancer_v2",
                    tvl_filter.clone(),
                    Some(balancer_pool_filter),
                )
                .exchange::<EVMPoolState<PreCachedDB>>(
                    "vm:curve",
                    tvl_filter.clone(),
                    Some(curve_pool_filter),
                )
                .exchange::<EkuboState>("ekubo_v2", tvl_filter.clone(), None)
                .exchange::<UniswapV4State>(
                    "uniswap_v4",
                    tvl_filter.clone(),
                    Some(uniswap_v4_pool_with_hook_filter),
                );
        }
        Chain::Base => {
            builder = builder
                .exchange::<UniswapV2State>("uniswap_v2", tvl_filter.clone(), None)
                .exchange::<UniswapV3State>("uniswap_v3", tvl_filter.clone(), None)
                .exchange::<UniswapV4State>(
                    "uniswap_v4",
                    tvl_filter.clone(),
                    Some(uniswap_v4_pool_with_hook_filter),
                )
        }
        Chain::Unichain => {
            builder = builder
                .exchange::<UniswapV2State>("uniswap_v2", tvl_filter.clone(), None)
                .exchange::<UniswapV3State>("uniswap_v3", tvl_filter.clone(), None)
                .exchange::<UniswapV4State>(
                    "uniswap_v4",
                    tvl_filter.clone(),
                    Some(uniswap_v4_pool_with_hook_filter),
                )
        }
        _ => {}
    }
    builder
}