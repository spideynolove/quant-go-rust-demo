# Solana DEX Arbitrage Bot - Design Document

**Date:** 2026-02-06
**Status:** Design Phase
**Timeline:** 1-2 weeks MVP

## Project Overview

A portfolio project demonstrating quantitative trading expertise for a Quantitative Engineer role. This system implements automated arbitrage detection and execution across Solana DEXs (Raydium and Orca) using Clean Architecture principles.

### Portfolio Context

This project complements **Gecko** (CEX Order Management System) to demonstrate:
- **Gecko**: CEX trading infrastructure, order management, production-grade OMS
- **This Project**: DEX/blockchain expertise, arbitrage strategies, Solana integration

Together, these projects cover the full scope of the target role: CEX/DEX arbitrage, low-latency execution, and multi-chain support.

## Design Decisions Summary

### Scope & Focus
- **Arbitrage Type**: Cross-DEX arbitrage on Solana (Raydium ↔ Orca)
- **Execution Mode**: Live testnet execution on Solana devnet
- **Trading Pairs**: Start with SOL/USDC (can expand later)
- **Architecture**: Monolithic async engine with Clean Architecture layers
- **Timeline**: 1-2 week MVP, production-ready later

**Rationale**: Starting with Solana-only reduces blockchain complexity (no cross-chain bridges), lower gas costs for testing, and faster iteration. Clean Architecture demonstrates enterprise-level design thinking.

### Technology Stack
- **Language**: Rust (demonstrates required Rust expertise)
- **Async Runtime**: Tokio (matches job requirements: "async Rust with Tokio")
- **Event Bus**: NATS (pub/sub for loose coupling)
- **Blockchain**: Solana SDK (`solana-client`, `solana-sdk`)
- **DEX SDKs**: `raydium-amm`, `orca-whirlpool`
- **WebSocket**: `tokio-tungstenite`
- **Serialization**: `serde`, `serde_json`
- **Logging**: `tracing` with structured logs

## Clean Architecture Design

### Layer Structure

```
┌───────────────────────────────────────┐
│        External Systems               │
│  (Raydium, Orca, Solana RPC, NATS)   │
└────────────────▲──────────────────────┘
                 │
    ┌────────────▼────────────────┐
    │   Interface Adapters        │
    │  - RaydiumPriceFeed         │
    │  - OrcaPriceFeed            │
    │  - SolanaExecutor           │
    │  - MetricsCollector         │
    └────────────▲────────────────┘
                 │
       ┌─────────▼──────────┐
       │   EVENT BUS (NATS) │
       │  - PriceUpdate     │
       │  - OpportunityDetected
       │  - TradeIntent     │
       │  - TradeFilled     │
       └─────────▲──────────┘
                 │
  ┌──────────────▼──────────────────┐
  │      Domain Services            │
  │  - OpportunityDetector          │
  │  - TradeValidator               │
  │  - ExecutionCoordinator         │
  │  - PositionTracker              │
  └──────────────▲──────────────────┘
                 │
       ┌─────────▼──────────┐
       │   Domain Model      │
       │  - Asset, Pool      │
       │  - ArbitrageOpportunity
       │  - Trade, Position  │
       │  - Price, Spread    │
       └─────────────────────┘
                 │
    ┌────────────▼────────────┐
    │   Infrastructure        │
    │  - main.rs (tokio)      │
    │  - config.toml          │
    │  - tracing/logging      │
    └─────────────────────────┘
```

### Core Components

#### 1. Domain Model (Pure Business Logic)
**Entities:**
- `Asset` - token info (symbol, mint address, decimals)
- `Pool` - DEX liquidity pool state
- `ArbitrageOpportunity` - detected price spread with profit calculation
- `Trade` - executed trade details
- `Position` - current holdings

**Value Objects:**
- `Price` - price with timestamp and source
- `Liquidity` - pool depth information
- `Spread` - price difference percentage
- `PnL` - profit/loss calculation

**Design Principle**: No I/O, no async, no external dependencies. Pure data structures and business logic.

#### 2. Domain Services (Use Cases)

**OpportunityDetector**
- Subscribes to: `PriceUpdate` events
- Logic: Compares prices from different DEXs, calculates spread
- Emits: `OpportunityDetected` events when spread > threshold

**TradeValidator**
- Subscribes to: `OpportunityDetected` events
- Logic: Validates profitability (fees, slippage), checks risk limits
- Emits: `TradeIntent` events for valid opportunities

**ExecutionCoordinator**
- Subscribes to: `TradeIntent` events
- Logic: Orchestrates trade execution sequence
- Emits: `ExecutionRequest` events to adapters

**PositionTracker**
- Subscribes to: `TradeFilled` events
- Logic: Maintains current positions, calculates realized PnL
- Emits: `PositionUpdate` events

#### 3. Interface Adapters

**RaydiumPriceFeed**
- WebSocket connection to Raydium API
- Parses Raydium-specific price updates
- Publishes normalized `PriceUpdate` events to NATS
- Handles reconnection automatically

**OrcaPriceFeed**
- WebSocket connection to Orca API
- Parses Orca-specific price updates
- Publishes normalized `PriceUpdate` events to NATS
- Handles reconnection automatically

**SolanaExecutor**
- Subscribes to: `ExecutionRequest` events
- Builds Solana transactions using DEX SDKs
- Signs and submits transactions via RPC
- Publishes `TradeFilled` or `TradeRejected` events

**MetricsCollector**
- Subscribes to: All events
- Logs metrics: opportunities detected, trades executed, PnL
- Outputs to: Console + JSON file

#### 4. Infrastructure

**main.rs**
- Initializes Tokio runtime
- Connects to NATS server
- Loads configuration from `config.toml`
- Spawns all adapter tasks
- Sets up graceful shutdown

**config.toml**
```toml
[network]
solana_rpc_url = "https://api.devnet.solana.com"
nats_url = "nats://localhost:4222"

[dexs.raydium]
websocket_url = "wss://..."
pool_address = "..."

[dexs.orca]
websocket_url = "wss://..."
pool_address = "..."

[strategy]
min_profit_threshold = 0.005  # 0.5%
max_trade_size_usd = 100.0

[wallet]
keypair_path = "/path/to/devnet-wallet.json"
```

## Data Flow

### Price Update Flow
```
Raydium WS → RaydiumPriceFeed → NATS[PriceUpdate]
                                      ↓
Orca WS → OrcaPriceFeed → NATS[PriceUpdate]
                                      ↓
                              OpportunityDetector
                                      ↓
                         NATS[OpportunityDetected]
                                      ↓
                               TradeValidator
                                      ↓
                            NATS[TradeIntent]
                                      ↓
                          ExecutionCoordinator
                                      ↓
                           NATS[ExecutionRequest]
                                      ↓
                              SolanaExecutor
                                      ↓
                            NATS[TradeFilled]
                                      ↓
                             PositionTracker
```

### Arbitrage Detection Logic

1. **Receive Price Updates**: Both DEXs publish price updates via NATS
2. **Calculate Spread**: `spread_pct = abs(price_a - price_b) / avg(price_a, price_b)`
3. **Estimate Costs**:
   - DEX fees: 0.25% - 0.30% per swap
   - Slippage: Estimate based on trade size vs pool depth
   - Gas fees: Solana transaction fees (~0.000005 SOL)
4. **Calculate Net Profit**: `net_profit_pct = spread_pct - total_costs_pct`
5. **Emit Signal**: If `net_profit_pct > threshold`, emit `OpportunityDetected`

### Transaction Execution

1. **Fetch Pool State**: Query current reserves from both DEXs
2. **Build Swap Instructions**:
   - Raydium: `swap_base_in` with slippage protection
   - Orca: Whirlpool swap instruction
3. **Assemble Transaction**:
   - Add compute budget instructions
   - Get recent blockhash
   - Sign with devnet wallet
4. **Submit in Parallel**: Send both transactions using `tokio::spawn`
5. **Confirm**: Track transaction status, handle retries
6. **Publish Result**: Emit `TradeFilled` or `TradeRejected`

## Error Handling

### Adapter-Level Errors
- WebSocket disconnections → Automatic reconnection with exponential backoff
- RPC failures → Retry with different RPC endpoint
- Transaction timeout → Rebuild with fresh blockhash

### Domain-Level Errors
- Slippage exceeded → Reject trade, log event
- Insufficient liquidity → Skip opportunity
- Risk limit violation → Halt execution, emit alert

### Recovery Strategies
- State snapshots: Persist positions to disk periodically
- Event replay: NATS stores events, can replay for recovery
- Graceful shutdown: Drain in-flight transactions before exit

## Monitoring & Metrics

### Key Metrics
- Opportunities detected per minute
- Trades executed (success rate)
- Average execution latency (detect → confirm)
- Realized PnL (per trade, cumulative)
- WebSocket connection health
- RPC latency

### Output Formats
- **Console**: Real-time log stream with `tracing`
- **JSON File**: Detailed trade log for analysis
- **Future**: Prometheus metrics, Grafana dashboard

## Testing Strategy

### Unit Tests
- Domain model: Pure functions, easy to test
- Domain services: Mock event bus, test business logic
- No external dependencies needed

### Integration Tests
- Adapter tests with mock WebSocket server
- SolanaExecutor with Solana test validator
- End-to-end with devnet

### Performance Tests
- Latency benchmarks: Price update → trade signal
- Load testing: Handle 100+ price updates/second
- Memory profiling: Ensure no leaks in long-running system

## Implementation Phases

### Phase 1: Foundation (Days 1-3)
- [ ] Project setup (Cargo workspace, dependencies)
- [ ] Domain model implementation
- [ ] NATS integration and event definitions
- [ ] Configuration management

### Phase 2: Market Data (Days 4-5)
- [ ] Raydium WebSocket adapter
- [ ] Orca WebSocket adapter
- [ ] Price normalization and event publishing
- [ ] OpportunityDetector service

### Phase 3: Execution (Days 6-8)
- [ ] TradeValidator service
- [ ] SolanaExecutor adapter
- [ ] Transaction building and signing
- [ ] Error handling and retries

### Phase 4: Monitoring & Polish (Days 9-10)
- [ ] MetricsCollector implementation
- [ ] PositionTracker service
- [ ] Comprehensive logging
- [ ] README and documentation

### Phase 5: Testing & Deployment (Days 11-14)
- [ ] Unit tests for domain logic
- [ ] Integration tests with devnet
- [ ] Performance benchmarks
- [ ] Deployment documentation

## Production Readiness Checklist

### Code Quality
- [ ] Clean Architecture principles enforced
- [ ] Error handling on all I/O operations
- [ ] Comprehensive logging with correlation IDs
- [ ] Unit test coverage > 80%
- [ ] Integration tests passing

### Operations
- [ ] Configuration via environment/file
- [ ] Graceful shutdown handling
- [ ] Health check endpoint
- [ ] Metrics exportable
- [ ] State recovery mechanism

### Documentation
- [ ] README with setup instructions
- [ ] Architecture diagram
- [ ] API/event documentation
- [ ] Deployment guide
- [ ] Performance benchmarks

### Security
- [ ] Wallet keypair securely managed
- [ ] No secrets in code
- [ ] RPC endpoint authentication
- [ ] Rate limiting on external calls

## Shared Learnings from Gecko

1. **State Machine Pattern**: Track order lifecycle clearly (PENDING → SUBMITTED → CONFIRMED/FAILED)
2. **Event-Driven Design**: Loose coupling via message bus scales better than direct calls
3. **Structured Logging**: Correlation IDs essential for debugging distributed systems
4. **Configuration Management**: External config files > hardcoded values
5. **Graceful Degradation**: System should handle partial failures (one DEX down, continue with others)

## Future Enhancements

### V2 Features (Post-MVP)
- [ ] Add more DEXs (Jupiter aggregator, Phoenix)
- [ ] Multi-pair arbitrage (SOL/USDC, SOL/USDT, etc.)
- [ ] Cross-chain arbitrage (Ethereum DEXs)
- [ ] Flash loan integration for capital efficiency
- [ ] Advanced risk management (position limits, circuit breakers)
- [ ] Backtesting framework using historical data
- [ ] Web dashboard for real-time monitoring

### Infrastructure Improvements
- [ ] Deploy to cloud (AWS/GCP) with Kubernetes
- [ ] CI/CD pipeline (GitHub Actions)
- [ ] Prometheus + Grafana monitoring
- [ ] Distributed tracing (Jaeger)
- [ ] High-availability setup (multiple instances)

## Success Criteria

### For Portfolio/Interview
- ✅ Clean, well-organized codebase demonstrating Clean Architecture
- ✅ Works on Solana devnet (can demo live)
- ✅ Shows understanding of arbitrage mechanics
- ✅ Demonstrates async Rust proficiency
- ✅ Professional documentation and README
- ✅ Performance metrics proving low-latency capability

### Technical Targets
- Latency: Price update → trade signal < 50ms
- Throughput: Handle 100+ price updates/second
- Uptime: Run continuously for 24+ hours without crashes
- Success rate: > 95% of valid opportunities successfully executed

## Repository Structure

```
quant-go-rust-demo/
├── docs/
│   ├── plans/
│   │   └── 2026-02-06-solana-arbitrage-design.md
│   └── architecture.md
├── src/
│   ├── domain/
│   │   ├── entities/
│   │   ├── services/
│   │   └── events.rs
│   ├── adapters/
│   │   ├── raydium_feed.rs
│   │   ├── orca_feed.rs
│   │   ├── solana_executor.rs
│   │   └── metrics_collector.rs
│   ├── infrastructure/
│   │   ├── config.rs
│   │   └── nats.rs
│   └── main.rs
├── tests/
│   ├── integration/
│   └── unit/
├── config.toml
├── Cargo.toml
└── README.md
```

## Next Steps

1. Review and validate this design
2. Set up development environment (NATS, Solana CLI)
3. Create implementation plan with detailed tasks
4. Begin Phase 1 implementation
5. Iterate and refine based on learnings

---

**Note**: This design prioritizes demonstrating Clean Architecture principles and production-ready thinking over quick-and-dirty prototyping. The goal is to showcase engineering maturity and system design skills to recruiters.
