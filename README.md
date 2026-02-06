# Solana DEX Arbitrage Bot

**Status:** 🔨 Design Phase | **Timeline:** 1-2 weeks MVP | **Target:** Quantitative Engineer Portfolio

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

### Current (Design Phase)
- ✅ Clean Architecture design complete
- ✅ Component specifications defined
- ✅ Event flow documented
- ✅ Technology stack selected

### Phase 1: Foundation (Days 1-3)
- [ ] Project setup with Cargo workspace
- [ ] Domain model implementation
- [ ] NATS integration and event definitions
- [ ] Configuration management

### Phase 2: Market Data (Days 4-5)
- [ ] Raydium WebSocket adapter
- [ ] Orca WebSocket adapter
- [ ] Price normalization
- [ ] Opportunity detection service

### Phase 3: Execution (Days 6-8)
- [ ] Trade validation service
- [ ] Solana transaction executor
- [ ] Error handling and retries
- [ ] Transaction confirmation tracking

### Phase 4: Monitoring (Days 9-10)
- [ ] Metrics collection service
- [ ] Position tracking
- [ ] PnL calculation
- [ ] Structured logging

### Phase 5: Testing & Docs (Days 11-14)
- [ ] Unit tests (domain logic)
- [ ] Integration tests (devnet)
- [ ] Performance benchmarks
- [ ] Deployment documentation

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

### Setup (Coming Soon)

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
cp config.example.toml config.toml
# Edit config.toml with your settings

# Run
cargo run --release
```

## Performance Targets

- **Latency:** Price update → trade signal < 50ms
- **Throughput:** Handle 100+ price updates/second
- **Uptime:** Run continuously 24+ hours without crashes
- **Success Rate:** > 95% of valid opportunities executed

## Documentation

- [Design Document](docs/plans/2026-02-06-solana-arbitrage-design.md) - Complete architectural design
- [Job Requirements](QuantJD.md) - Target role specifications
- Architecture Diagram (coming soon)
- API/Event Documentation (coming soon)

## Development Progress

Track implementation progress in [GitHub Issues](../../issues) and [Project Board](../../projects).

**Last Updated:** 2026-02-06

## License

MIT License - See [LICENSE](LICENSE)

## Contact

This is a portfolio project demonstrating quantitative trading system expertise. For questions or collaboration, please open an issue.

---

**Note:** This project uses Solana devnet for safe testing. No mainnet funds are at risk during development.
