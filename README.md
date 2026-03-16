# Solana DEX Arbitrage Bot

**Status:** ✅ Implementation Complete | **Timeline:** 1-2 weeks MVP | **Target:** Quantitative Engineer Portfolio

A production-grade automated arbitrage system for Solana DEXs, demonstrating Clean Architecture principles, low-latency execution, and event-driven design.

## Overview

This project implements automated arbitrage detection and execution across Solana decentralized exchanges (Raydium and Orca). It showcases:

- **Clean Architecture** with clear separation of concerns
- **Event-driven design** using NATS message bus
- **Async Rust** with Tokio for low-latency performance
- **Blockchain integration** via Solana SDK
- **Production-ready patterns** (error handling, monitoring, testing)

### Portfolio Context

This project is part of a comprehensive trading systems portfolio:

| Project | Focus | Technologies |
|---------|-------|-------------|
| **Gecko** | CEX Order Management System | Go/Rust, NATS, HFT patterns |
| **This Project** | DEX Arbitrage on Solana | Rust, Tokio, Solana, Event-driven |

Together, these demonstrate expertise in both centralized and decentralized trading infrastructure.

## Architecture

The system follows **Clean Architecture / Hexagonal Architecture** principles:

```
External Systems (DEXs, Blockchain)
         ↓
Interface Adapters (WebSocket feeds, Executors)
         ↓
Event Bus (NATS pub/sub)
         ↓
Domain Services (Opportunity Detection, Validation, Execution)
         ↓
Domain Model (Entities, Value Objects)
```

**Key Design Principles:**
- Domain logic independent of external systems
- Dependencies point inward
- Event-driven communication for loose coupling
- Testable architecture with mockable adapters

See [Design Document](docs/plans/2026-02-06-solana-arbitrage-design.md) for full architectural details.

## Features

### Completed
- ✅ Clean Architecture implementation
- ✅ Domain model with entities and value objects
- ✅ NATS event-driven messaging
- ✅ Raydium and Orca WebSocket adapters
- ✅ Opportunity detection service
- ✅ Trade validation with risk limits
- ✅ Solana transaction executor
- ✅ Position tracking and PnL calculation
- ✅ Metrics collection with JSON output
- ✅ Structured logging and graceful shutdown
- ✅ Unit tests for domain logic
- ✅ Integration tests

### Phase 1: Foundation (Days 1-3)
- ✅ Project setup with Cargo workspace
- ✅ Domain model implementation
- ✅ NATS integration and event definitions
- ✅ Configuration management

### Phase 2: Market Data (Days 4-5)
- ✅ Raydium WebSocket adapter
- ✅ Orca WebSocket adapter
- ✅ Price normalization
- ✅ Opportunity detection service

### Phase 3: Execution (Days 6-8)
- ✅ Trade validation service
- ✅ Solana transaction executor
- ✅ Error handling and retries
- ✅ Transaction confirmation tracking

### Phase 4: Monitoring (Days 9-10)
- ✅ Metrics collection service
- ✅ Position tracking
- ✅ PnL calculation
- ✅ Structured logging

### Phase 5: Testing & Docs (Days 11-14)
- ✅ Unit tests (40 passing)
- ✅ Integration tests (4 passing)
- ✅ Documentation
- ✅ Criterion performance benchmarks

### Phase 6: Real Swap Execution (2026-03-17)
- ✅ Pool config layer (Raydium AMM v4 + Orca Whirlpool)
- ✅ Runtime pool-state fetching (nonce, tick index)
- ✅ Raydium AMM v4 swap instruction builder
- ✅ Orca Whirlpool swap instruction builder with tick array PDA derivation
- ✅ SolanaExecutor wired with real swap instructions
- ✅ ATA derivation (inlined, no external crate)

## Technology Stack

- **Language:** Rust (async with Tokio)
- **Event Bus:** NATS (pub/sub messaging)
- **Blockchain:** Solana (devnet for testing)
- **DEXs:** Raydium, Orca
- **WebSocket:** tokio-tungstenite
- **Logging:** tracing with structured logs
- **Testing:** Solana test validator, mock adapters

## Quick Start

### Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Solana CLI
sh -c "$(curl -sSfL https://release.solana.com/stable/install)"

# Install NATS server
# macOS
brew install nats-server

# Linux
curl -L https://github.com/nats-io/nats-server/releases/download/v2.10.7/nats-server-v2.10.7-linux-amd64.tar.gz | tar -xz
sudo mv nats-server-v2.10.7-linux-amd64/nats-server /usr/local/bin/
```

### Setup

```bash
# Clone repository
git clone <repo-url>
cd quant-go-rust-demo

# Start NATS server
nats-server &

# Create devnet wallet
solana-keygen new --outfile ~/.config/solana/devnet-wallet.json

# Airdrop SOL for testing
solana airdrop 2 --url devnet

# Configure
cp config.toml.example config.toml
# Edit config.toml with your settings

# Run
cargo run --release
```

## Configuration

Edit `config.toml`:

```toml
[nats]
url = "nats://localhost:4222"

[solana]
rpc_url = "https://api.devnet.solana.com"
ws_url = "wss://api.devnet.solana.com"
keypair_path = "/path/to/keypair.json"
commitment = "confirmed"

[trading]
min_spread_bps = 50
min_profit_usd = 10.0
max_position_size = 1000.0
slippage_tolerance_bps = 10

[risk]
max_open_positions = 5
max_daily_loss = 100.0
circuit_loss_threshold = 50.0

[logging]
level = "info"
format = "json"
```

## Testing

```bash
# Unit tests
cargo test

# Integration tests
cargo test --test integration_test

# Run specific test
cargo test test_spread_bps
```

## Performance Targets

- **Latency:** Price update → trade signal < 50ms
- **Throughput:** Handle 100+ price updates/second
- **Uptime:** Run continuously 24+ hours without crashes
- **Success Rate:** > 95% of valid opportunities executed

## Metrics

Metrics are written to `metrics.jsonl` in JSON Lines format:

```json
{"timestamp":1234567890,"opportunities_detected":42,"trades_executed":5,"trades_rejected":2,"total_pnl_realized":125.50,"total_fees_paid":12.50,"net_profit":113.00}
```

## Event Flow

```
PriceUpdate (Raydium/Orca)
    ↓
OpportunityDetector
    ↓
OpportunityDetected
    ↓
TradeValidator
    ↓
TradeIntent
    ↓
ExecutionCoordinator
    ↓
ExecutionRequest
    ↓
SolanaExecutor
    ↓
TradeFilled / TradeRejected
    ↓
PositionTracker
    ↓
PositionUpdate
```

## Documentation

- [Design Document](docs/plans/2026-02-06-solana-arbitrage-design.md) - Complete architectural design
- [Job Requirements](QuantJD.md) - Target role specifications
- Architecture Diagram (coming soon)
- API/Event Documentation (coming soon)

## Benchmarks

Instruction construction latency (measured with Criterion on release build):

| Operation | Time |
|-----------|------|
| `build_raydium_swap_ix` | ~158 ns |
| `build_orca_swap_ix` | ~113 ns |
| `derive_tick_arrays` (PDA x3) | ~21 µs |

Run benchmarks: `cargo bench`

## Development Progress

Real swap execution complete. Bot constructs valid Raydium AMM v4 and Orca Whirlpool swap instructions with correct byte layout, account ordering, and PDA derivation. Ready for devnet simulation.

**Implementation Complete:** 2026-03-17
**Unit Tests:** 40 passing
**Integration Tests:** 4 passing
**Benchmarks:** 3 benchmarks (instruction build latency sub-200ns)

## License

MIT License - See [LICENSE](LICENSE)

## Contact

This is a portfolio project demonstrating quantitative trading system expertise. For questions or collaboration, please open an issue.

---

**Note:** This project uses Solana devnet for safe testing. No mainnet funds are at risk during development.
