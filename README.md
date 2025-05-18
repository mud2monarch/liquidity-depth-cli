# Liquidity Depth CLI Tool

A command-line tool to compute the **2% depth** of on-chain liquidity for a given token pair and protocol, using the [Tycho Simulation](https://github.com/propeller-heads/tycho-simulation) framework.

## What It Does

For a given `token_in`, `token_out`, and protocol (e.g. `uniswap_v3`, `balancer_v2`), this tool simulates swaps using Tycho to determine:

- The maximum amount of `token_in` that can be swapped
- Before slippage exceeds 2% relative to the current spot price

The tool performs a binary search over input sizes and logs results to disk.

## Why It’s Useful

Liquidity depth is critical for:

- Traders seeking to estimate price impact before executing large swaps
- Protocol researchers analyzing cross-DEX liquidity profiles
- MEV solvers and routing engines evaluating execution viability

This tool gives a fast, scriptable way to inspect protocol-level liquidity without needing custom integrations or node infrastructure.

## Features

- Fast REVM-based swap simulation via Tycho
- Slippage-aware binary search for 2% depth
- Logs results in structured JSONL format
- Optional TUI for interactive usage (WIP)

## Getting Started

```bash
cargo run --release
