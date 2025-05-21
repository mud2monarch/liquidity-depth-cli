use std::{env, str::FromStr};
use num_bigint::BigUint;
use tycho_common::{
    models::Chain,
    Bytes,
};
use num_bigint::ToBigUint;
use tycho_simulation::{
    evm::{
        engine_db::tycho_db::PreCachedDB,
        protocol::{
            ekubo::state::EkuboState, filters::{balancer_pool_filter, curve_pool_filter, uniswap_v4_pool_with_hook_filter}, u256_num::u256_to_biguint, uniswap_v2::state::UniswapV2State, uniswap_v3::state::UniswapV3State, uniswap_v4::state::UniswapV4State, vm::state::EVMPoolState
        },
        stream::ProtocolStreamBuilder,
    }, models::Token, protocol::models::BlockUpdate, tycho_client::feed::component_tracker::ComponentFilter, utils::load_all_tokens
};
use futures::StreamExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── env / CLI boilerplate ──────────────────────────────────────────────────
    let chain = Chain::Unichain;
    let tycho_url = env::var("TYCHO_URL")
        .unwrap_or_else(|_| "tycho-unichain-beta.propellerheads.xyz".into());
    let tycho_api_key =
        env::var("TYCHO_API_KEY").unwrap_or_else(|_| "sampletoken".into());

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
        "0x4200000000000000000000000000000000000006",
        6,
        "USDC",
        10_000.to_biguint().unwrap()
    );

    let weth = Token::new(
        "0x078D782b760474a361dDA0AF3839290b0EF57AD6",
        18,
        "WETH",
        10_000.to_biguint().unwrap()
    );

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

    let mut blocks_seen = 0;
    while let Some(msg) = stream.next().await {
        let block = msg?; // Result<BlockUpdate>
        blocks_seen += 1;

        println!("Block #{}", block.block_number);
        println!("   → {} states", block.states.len());
        println!("   → {} new pairs", block.new_pairs.len());
        println!("   → {} removed pairs", block.removed_pairs.len());
        
        // Print first few states for debugging
        for (_id, state) in block.states.iter() {
            // println!("{:#?}", state); Not doing full debugging for this
            let out = state.get_amount_out(
                u256_to_biguint(weth.one()),
                &weth,
                &usdc).unwrap().amount;
            println!(" 1 WETH = {} USDC", out);
        }
        
        if blocks_seen >= 20 {
            println!("Seen {} blocks", blocks_seen);
            break;
        }
    }

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
    //         let spot = state.spot_price(weth, usdc)?;
    //         println!("🟢 spot_price 1 WETH → {spot:.6} USDC");

    //         // amount-out for 1 WETH
    //         let one_weth = BigUint::from(1_000_000_000_000_000_000u128);
    //         let out = state.get_amount_out(one_weth.clone(), weth, usdc)?;
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