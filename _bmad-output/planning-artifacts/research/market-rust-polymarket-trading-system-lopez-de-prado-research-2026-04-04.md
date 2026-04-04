---
stepsCompleted: [1, 2, 3, 4, 5, 6]
inputDocuments:
  - https://github.com/Polymarket/rs-clob-client
  - https://raw.githubusercontent.com/Polymarket/rs-clob-client/main/Cargo.toml
  - https://raw.githubusercontent.com/Polymarket/rs-clob-client/main/README.md
  - https://docs.polymarket.com/market-data/websocket/overview
  - https://docs.polymarket.com/market-data/websocket/market-channel
  - https://docs.polymarket.com/market-data/websocket/user-channel
  - https://docs.polymarket.com/trading/orderbook
  - https://docs.polymarket.com/trading/orders/create
  - https://docs.polymarket.com/trading/orders/cancel
  - https://docs.polymarket.com/market-makers/trading
  - https://docs.polymarket.com/trading/fees
  - https://docs.polymarket.com/market-makers/maker-rebates
  - https://docs.polymarket.com/market-makers/liquidity-rewards
  - https://docs.polymarket.com/market-data/fetching-markets
  - https://docs.polymarket.com/trading/orders/attribution
  - https://docs.polymarket.com/builders/overview
  - https://docs.polymarket.com/trading/gasless
  - https://docs.polymarket.com/api-reference/geoblock
  - https://www.quantresearch.org/Innovations.htm
  - https://www.quantresearch.org/Publications.htm
  - https://hudsonthames.org/fractional-differentiation/
  - https://hudsonthames.org/does-meta-labeling-add-to-signal-efficacy-triple-barrier-method/
  - https://hudsonthames.org/machine-learning-trading-essentials-part-2-fractionally-differentiated-features-filtering-and-labelling/
  - https://arxiv.org/abs/1806.05101
workflowType: 'research'
lastStep: 6
research_type: 'technical'
research_topic: 'Profitable Rust CLOB trading system on Polymarket using only polymarket-client-sdk and Lopez de Prado methods'
research_goals: 'Deliver a no-code, step-by-step BMAD plan for a Rust trading system on an easier-to-profit Polymarket segment, while satisfying ws+clob features, market/user websockets, full CLOB lifecycle, Builder/gasless path, tokio async architecture, and reliability requirements.'
user_name: 'Sang'
date: '2026-04-04'
web_research_enabled: true
source_verification: true
---

# Research Report: technical

**Date:** 2026-04-04
**Author:** Sang
**Research Type:** technical

---

## Research Overview

This research evaluates how to build a **Rust-first, no-code-planned Polymarket trading system** focused on practical profitability under real exchange constraints, while using **only the official `polymarket-client-sdk`** for Rust trading integration. The strategy direction chosen is not “heroic directional prediction,” but **rebate-and-incentive-assisted market making** in high-activity sports/event categories, where structure and recurring flow make model learning and inventory recycling more reliable.

Current public documentation supports the full core requirement set for a Rust CLOB engine: `clob` + `ws` features, market and user WebSocket channels, order lifecycle (limit/market, cancel, batch), and authenticated builder-attributed order flow. However, full relayer-based gasless wallet operations are presently documented primarily via Builder/Relayer SDKs outside Rust; therefore, this plan treats gasless support as **possible through Builder routing and optional relayer integration architecture**, not as a guaranteed pure-Rust feature end-to-end.

Methodology combined: (1) official Polymarket SDK/docs verification, (2) microstructure and market-maker economics review from Polymarket fee/reward mechanics, and (3) López de Prado methodological mapping (labeling, validation, overfitting control, and bet sizing) into a production-ready BMAD execution plan with explicit kill-switch and statistical validation gates.

---

## Technical Research Scope Confirmation

**Research Topic:** Profitable Rust CLOB trading system on Polymarket using only `polymarket-client-sdk` and López de Prado methods.

**Research Goals:**
- Find a practical “easier-to-profit” market segment on Polymarket.
- Design a no-code BMAD delivery plan for a Rust trading system.
- Enforce architecture constraints: `ws`+`clob`, market/user websockets, full CLOB execution lifecycle, async tokio design, robust reconnection/heartbeat/error handling.
- Assess builder/gasless feasibility without violating official SDK constraints.

**Scope Confirmed:**
- Architecture analysis
- Implementation sequencing
- Technology + integration patterns
- Performance and reliability considerations
- Research-grade validation protocol to reduce false positives


## Technology Stack Analysis

### Core Runtime and SDK

- Official crate: `polymarket-client-sdk`.
- Verified crate version in upstream `Cargo.toml`: `0.4.4` with Rust edition 2024 and MSRV 1.88.
- Required feature flags for this project are validated: `clob`, `ws`.
- Optional but strongly relevant operational feature: `heartbeats`.

### What the official Rust SDK already supports

- CLOB authentication flows (standard and builder-authenticated).
- Typed order builders (market/limit) and signed submission.
- Batch operations (`post_orders`, `cancel_orders`).
- WebSocket market stream examples (orderbook + updates) and user stream examples (orders/trades).
- Builder promotion / attribution flow in Rust examples.

### Infrastructure fit for your DigitalOcean deployment

- A single droplet is sufficient for research and paper/live-small operation when architecture is event-driven and low-latency local state is in memory.
- Recommended split is process-level, not host-level, initially: ingestion loop, signal loop, execution loop, risk loop, and telemetry loop.


## Integration Patterns Analysis

### Market WebSocket (public market data)

- Endpoint: `wss://ws-subscriptions-clob.polymarket.com/ws/market`.
- Relevant events for strategy/quoting: `book`, `price_change`, `last_trade_price`, `tick_size_change`, and optionally `best_bid_ask` when custom feature is enabled.
- Dynamic subscribe/unsubscribe is supported and should be used for rotating market universe.

### User WebSocket (authenticated order/fill lifecycle)

- Endpoint: `wss://ws-subscriptions-clob.polymarket.com/ws/user`.
- Auth payload requires API key, secret, passphrase.
- Core events: `order` and `trade` with lifecycle statuses (e.g., `MATCHED`, `MINED`, `CONFIRMED`, retry/fail states).

### Heartbeats and session liveness

- WebSocket docs require periodic ping/pong behavior (market/user channels).
- Trading docs also expose order-session heartbeat semantics where missing heartbeats can cancel open orders.
- SDK `heartbeats` feature + separate operational watchdog should be treated as mandatory reliability controls.

### Full CLOB lifecycle support (confirmed)

- Order creation: limit (GTC/GTD), market-like execution via FOK/FAK, post-only behavior.
- Cancellations: single, multiple, by market/token, and global cancel.
- Batch placement supported (up to documented limits).


## Architectural Patterns and Design

### Recommended async topology (Tokio-native)

1. **Market Ingestion Service**: consumes market WS, normalizes book/trade/tick-size updates.
2. **User State Service**: consumes authenticated user WS, reconciles order/trade state.
3. **Signal Service**: computes fair values, edge scores, and inventory-aware quote skew.
4. **Execution Service**: places/cancels/batches orders via CLOB API.
5. **Risk Service**: kill-switch, max exposure, stale-data guard, geoblock/availability gate.
6. **Persistence/Telemetry Service**: writes event logs, PnL attribution, and model diagnostics.

Use bounded channels with backpressure between services to avoid cascading lag.

### Reliability architecture

- Connection supervisors with exponential backoff + jitter.
- Independent watchdog timers for market stream freshness and user stream freshness.
- Automatic “cancel-all on uncertainty” rules when any of: stale book, auth failure, heartbeat miss, or inventory breach.
- Tick-size-change listener integrated into quote generation to avoid rejected orders.


## Implementation Approaches and Technology Adoption

### “Easiest-to-profit” market choice (practical, not guaranteed)

**Recommended segment:** **Top-liquidity sports sub-universe** (not all sports markets), initially pre-game then selective in-play.

**Why this is the most practical path:**
- Live sampling shows Sports with the largest 24h aggregate flow (~46.9M), but also heavy long-tail fragmentation; therefore strategy should only trade the top-liquidity decile of events.
- Polymarket publishes substantial liquidity incentive pools in sports/esports categories (April 2026 scope), improving maker economics on qualified markets.
- Makers can receive both liquidity incentives and fee-funded maker rebates in eligible categories.
- Frequent, repeated event structure helps model calibration and risk-policy standardization.
- High participation in selected top events improves fill probability and reduces one-sided inventory traps.

**Empirical category context (active/open sample):**
- Sports: 5,082 events, total 24h volume ≈ 46.9M, but `vol>=1k` ratio ≈ 11.2% (fragmented)
- Politics: 1,422 events, total 24h volume ≈ 39.7M, `liq>=10k` ratio ≈ 83.5% (deep but crowded)
- Weather: 260 events, total 24h volume ≈ 4.07M, `vol>=1k` ratio ≈ 71.5% and `liq>=10k` ratio ≈ 87.3% (excellent concentration)

This leads to a practical launch rule: **primary = top-liquidity sports**, **fallback/secondary = weather markets** when sports microstructure quality degrades.

**Profitability framing:**

Expected edge per unit flow should be treated as:

$E[\text{edge}] = E[\text{spread capture}] + E[\text{maker rebates}] + E[\text{liquidity incentives}] - E[\text{adverse selection}] - E[\text{inventory carry}] - E[\text{ops/infra}]$

If this quantity is persistently negative in out-of-sample rolling windows, strategy must auto-degrade or stop.

### Gasless via Builder: feasibility assessment

- **Confirmed in Rust:** builder-attributed CLOB request flow and builder-authenticated operations.
- **Documented for full relayer gasless wallet ops:** primarily via Builder/Relayer SDKs in TypeScript/Python docs.
- Therefore, for strict Rust-only execution, treat gasless as **partially available** (attribution + builder-connected flow) unless you add an approved relayer integration boundary.


## López de Prado Method Mapping (Practical Set)

The following method stack is recommended for this system:

1. **Event-based sampling** (dollar/volume bars mindset) to reduce time-bar noise.
2. **CUSUM filtering** for event detection and whipsaw control.
3. **Fractional differentiation** to preserve memory while achieving stationarity.
4. **Triple-barrier labeling** for realistic path-dependent labels.
5. **Meta-labeling** to decide whether to act on primary signals.
6. **Bet sizing / corrective AI** for position scaling under uncertainty.
7. **Purged K-fold + embargo** to reduce leakage.
8. **CPCV** for multi-path robustness.
9. **Sample uniqueness / sequential bootstrap** for dependent-label datasets.
10. **PSR / DSR / MinTRL / PBO discipline** for overfitting and false discovery control.
11. **False Strategy theorem awareness**: high in-sample Sharpe alone is never sufficient.


## Step-by-Step BMAD Plan (No Code)

### B — Business and market operating plan

1. **Compliance and availability gate first**
	- Run geoblock checks and legal eligibility for your deployment region before any live trading.
	- Exit criterion: trading region confirmed not blocked.

2. **Define profit objective as a decomposition, not a single PnL number**
	- Target spread capture + rebates + incentives net of adverse selection and inventory costs.
	- Exit criterion: measurable edge components and acceptable downside envelope.

3. **Universe policy for launch**
	- Start only with high-volume, high-liquidity, fee-enabled sports events in the top-liquidity decile; use weather as secondary deployment universe.
	- Exit criterion: ranked universe with daily rotation rules.

### M — Model and research protocol

4. **Create event-driven research dataset**
	- Book states, mid/spread dynamics, trade aggressor proxies, queue-position features, inventory state.
	- Exit criterion: reproducible event dataset with latency-aligned timestamps.

5. **Build primary signal families**
	- Micro-mean-reversion around book imbalance and spread transitions.
	- Catalyst regime filters (pre-game vs in-play).
	- Exit criterion: stable baseline with positive gross expectancy in walk-forward.

6. **Apply triple-barrier + meta-labeling**
	- Primary model predicts side; meta model predicts whether to trade / how much.
	- Exit criterion: improved precision and risk-adjusted performance vs primary-only.

7. **Validation hardening**
	- Purged CV + embargo + CPCV + DSR/PSR/MinTRL checks.
	- Exit criterion: strategy passes statistical significance and robustness thresholds.

### A — Architecture and production design

8. **Implement async service boundaries (Tokio)**
	- Separate ingest, signal, execution, risk, and telemetry loops.
	- Exit criterion: no single loop can block all others.

9. **Implement reliability controls before speed optimization**
	- Reconnect supervisors, heartbeat watchdogs, stale-feed guards, tick-size-change handlers.
	- Exit criterion: failure drills complete with deterministic safe-state behavior.

10. **Builder/gasless integration decision**
	 - If strict Rust-only, use builder attribution and document gasless limits.
	 - If full gasless is mandatory, add an approved relayer boundary as a controlled integration path.
	 - Exit criterion: explicit architecture decision and test plan.

### D — Delivery, deployment, and iteration

11. **Three-stage rollout**
	 - Replay simulation → paper trading → micro-capital live.
	 - Capital ladder increases only after out-of-sample and live metrics agree.

12. **Ongoing adaptive operations**
	 - Daily model diagnostics, drift checks, and auto-degrade policies.
	 - Weekly universe refresh and parameter sanity checks.
	 - Monthly full retraining only if robustness gates remain green.


## Requirement Coverage Matrix

- `Cargo.toml` with features `ws` and `clob`: **Supported and verified** in official crate feature model.
- Market WebSocket (orderbook + price updates): **Supported** (`market` channel, `book`, `price_change`, `last_trade_price`, etc.).
- User WebSocket (authenticated orders & fills): **Supported** (`user` channel with auth, `order` and `trade` lifecycle events).
- Full CLOB trading (limit/market/cancel/batch): **Supported** via create/cancel docs and SDK capabilities.
- Gasless via Builder if possible: **Partially supported in Rust path; full relayer-based gasless operations are documented mainly via non-Rust relayer clients**.
- Clean async architecture with tokio: **Feasible and recommended**.
- Graceful reconnection, heartbeats, error handling: **Required and feasible**, with explicit docs and SDK support patterns.


## Risk Assessment and Mitigation

- **Adverse selection risk (live markets):** tighten spread only where fill quality supports it; dynamic skew by inventory.
- **Model decay risk:** enforce rolling DSR/PSR gates; degrade to passive mode when confidence drops.
- **Operational risk:** kill switch + cancel-all on stale state or auth anomalies.
- **Regulatory/geoblock risk:** mandatory pre-trade geoblock check.
- **False discovery risk:** no promotion to live scale without CPCV + DSR + MinTRL thresholds.


## Strategic Recommendation

If your objective is “most practical path to early profitability,” prioritize:

1. Incentivized, high-flow sports market making,
2. Inventory-neutral execution discipline,
3. López de Prado validation stack before capital scaling,
4. Reliability-first engineering (heartbeat/reconnect/risk controls),
5. Builder attribution now; full gasless boundary only when operationally justified.


## Technical Research Methodology and Source Verification

### Primary sources used

- Official Rust SDK repository and raw files
- Official Polymarket developer docs (WebSocket, order lifecycle, fees/rebates, builder program, gasless docs, geoblock)
- López de Prado official innovation/publication pages
- Supporting implementation-oriented research notes (Hudson & Thames)
- Microstructure evidence source (arXiv order-book market-making study)

### Limitations

- Some academic/publisher pages and PDFs were not machine-extractable via this environment (access/format restrictions).
- For those items, conclusions were anchored only where text was directly accessible or corroborated elsewhere.


## Research Conclusion

This system is viable with a clear path to production under your constraints, and the highest-probability operational edge is **incentive-aware market making in a carefully filtered top-liquidity sports sub-universe**, with **weather as the secondary expansion/fallback universe** due strong event-level concentration. The official Rust SDK already covers your core CLOB + WebSocket requirements. Profitability credibility hinges less on coding speed and more on strict anti-overfitting protocol, robust async reliability, and disciplined rollout gates.

**Research Completion Date:** 2026-04-04  
**Source Verification:** Completed against current public docs and accessible research sources  
**Confidence:** High for SDK/architecture/ops feasibility; medium-high for profitability assumptions (market conditions remain non-stationary)