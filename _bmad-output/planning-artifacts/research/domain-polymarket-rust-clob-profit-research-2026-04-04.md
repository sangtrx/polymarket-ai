---
stepsCompleted: [1, 2, 3, 4, 5, 6]
inputDocuments: []
workflowType: 'research'
lastStep: 6
research_type: 'domain'
research_topic: 'Profitable Polymarket Rust CLOB strategy using López de Prado methods'
research_goals: 'Design a no-code, step-by-step BMAD plan for a profitable Rust trading system on Polymarket using only the official polymarket-client-sdk and meeting websocket/CLOB/gasless/reliability requirements.'
user_name: 'Sang'
date: '2026-04-04'
web_research_enabled: true
source_verification: true
---

# Research Report: domain

**Date:** 2026-04-04  
**Author:** Sang  
**Research Type:** domain

---

## Research Overview

This research evaluates where a Rust-based Polymarket strategy can most realistically reach **positive expected value** with the least modeling burden, then maps that opportunity to a **no-code BMAD delivery plan** constrained to the official Rust SDK (`polymarket-client-sdk`) and official API/docs.

Primary finding: the most practical “easy-profit” entry is **incentivized, two-sided market making** in reward/rebate-eligible markets (especially sports/esports where incentive pools are explicit), then layering López de Prado methods for quote selection, risk throttling, and overfitting control. This is easier than pure directional forecasting because PnL can come from a mix of spread capture, maker rebates, and liquidity rewards (while controlling inventory risk).

For implementation, the official stack supports required capabilities: CLOB order operations, market+user WebSocket streams, heartbeat/session safety, builder attribution, and gasless relayer workflows (Builder/Relayer auth). The plan below sequences these into BMAD phases from research to production hardening.

---

## Domain Research Scope Confirmation

**Research Topic:** Profitable Polymarket Rust CLOB strategy using López de Prado methods  
**Research Goals:** No-code BMAD plan, Rust-first, official SDK only, production reliability.

**Scope covered:**
- Industry/market structure for Polymarket maker workflows
- Incentive mechanics (maker rebates + liquidity rewards)
- Regulatory/availability constraints relevant to deployment
- Technical architecture requirements (SDK, WS, CLOB, gasless, resilience)
- Quant methodology overlay using López de Prado framework

---

## Industry Analysis

### Market Structure and Profit Drivers

Polymarket’s CLOB supports orderbook-driven execution and maker/taker dynamics, with read endpoints public and trading endpoints authenticated.  
_Source: https://docs.polymarket.com/api-reference/introduction_  
_Source: https://docs.polymarket.com/api-reference/authentication_

Maker-side economics are explicitly supported by:
1. **Spread capture** (classical market making),
2. **Maker rebates** (daily USDC, fee-curve weighted),
3. **Liquidity rewards** (formulaic scoring + per-market reward pools).

This creates a practical path where alpha does not rely only on directional prediction.

### Why This Is a Practical “Easy-Profit” Entry

The easiest documented edge is **incentivized passive liquidity provision** in markets where:
- reward rates are active,
- spreads are wide enough to monetize,
- fill rates are adequate,
- inventory can be neutralized quickly.

Official docs expose filters/fields for this selection (`spread`, `volume_24hr`, competitiveness, reward configs).  
_Source: https://docs.polymarket.com/api-reference/rewards/get-multiple-markets-with-rewards_  
_Source: https://docs.polymarket.com/api-reference/markets/get-sampling-markets_  
_Source: https://docs.polymarket.com/api-reference/markets/get-sampling-simplified-markets_

---

## Competitive Landscape (Operational, Not Vendor)

### Execution Constraints You Must Outperform

- Matching and lifecycle semantics are explicit (`live`, `matched`, `delayed`, `unmatched`; trade states `MATCHED/MINED/CONFIRMED/RETRYING/FAILED`).  
  _Source: https://docs.polymarket.com/concepts/order-lifecycle_
- Sports markets can add behavior complexity (e.g., matching delay; clear-book behavior around starts), requiring tighter controls.  
  _Source: https://docs.polymarket.com/trading/orders/create_  
  _Source: https://docs.polymarket.com/concepts/order-lifecycle_
- Reliability and backoff behavior are crucial because 425/429/5xx paths are documented and expected in production.  
  _Source: https://docs.polymarket.com/resources/error-codes_  
  _Source: https://docs.polymarket.com/api-reference/rate-limits_

### Strategic Positioning

Instead of trying to “predict outcomes better than everyone” on day 1, start with:
- **Execution quality + incentives + risk discipline**,
- Then add predictive overlays (meta-labeling/position sizing) once execution KPIs are stable.

---

## Regulatory and Deployment Constraints

### Geographic Availability

Polymarket exposes geoblocking checks and blocked/close-only regions. This must be enforced pre-trade.  
_Source: https://docs.polymarket.com/api-reference/geoblock_

### Builder/Gasless Constraints

- Gasless relayer operations are available via Builder or Relayer credentials.
- EOA wallets do not get relayer benefits in the same way as Safe/Proxy workflows.

_Source: https://docs.polymarket.com/builders/overview_  
_Source: https://docs.polymarket.com/trading/gasless_  
_Source: https://docs.polymarket.com/builders/tiers_

---

## Technical Trends and Implementation Readiness

### Official SDK Fitness (Rust)

The official Rust crate supports:
- `clob` feature for trading/auth/order builders,
- `ws` feature for market/user streaming,
- builder-authenticated flow and promote-to-builder path,
- batch order/cancel operations, rewards/balance APIs.

_Source: https://github.com/Polymarket/rs-clob-client_  
_Source: https://crates.io/crates/polymarket-client-sdk_

### Required Reliability Controls Are Officially Documented

- Market WS + User WS channels with ping/pong cadence and subscription updates.  
  _Source: https://docs.polymarket.com/api-reference/wss/market_  
  _Source: https://docs.polymarket.com/api-reference/wss/user_
- Heartbeat endpoint to keep order session alive; missing heartbeats can trigger mass cancel safety behavior.  
  _Source: https://docs.polymarket.com/api-reference/trade/send-heartbeat_  
  _Source: https://docs.polymarket.com/trading/orders/overview_
- Error taxonomy and retry semantics (429 backoff, 425 restart windows, cancel-only states).  
  _Source: https://docs.polymarket.com/resources/error-codes_  
  _Source: https://docs.polymarket.com/api-reference/rate-limits_

---

## López de Prado Method Mapping (No-Code Research Plan)

> Note: Core methods are from *Advances in Financial Machine Learning* and related practitioner research implementations. Direct journal/book pages are partially paywalled, so open references are used where accessible.

### Methods to Apply in Sequence

1. **Event-based sampling** (replace naïve clock time with trade/orderbook event clocks).  
2. **CUSUM event filter** (reduce noisy retriggers).  
3. **Triple-barrier labeling** (profit-take / stop-loss / vertical barrier outcomes).  
4. **Meta-labeling** (secondary model decides *whether* to act on primary signal).  
5. **Probability calibration + bet sizing** (map confidence to quote skew/size).  
6. **Fractional differentiation** for features that preserve memory with stationarity improvements.  
7. **Purged + embargoed CV**, then **CPCV** for robust out-of-sample distributions.

_Open references:_  
_Source: https://hudsonthames.org/does-meta-labeling-add-to-signal-efficacy-triple-barrier-method/_  
_Source: https://github.com/hudson-and-thames/meta-labeling_  
_Source: https://en.wikipedia.org/wiki/Purged_cross-validation_  
_Source: https://hudsonthames.org/fractional-differentiation/_

---

## Recommended “Easy-Profit” Market to Start

## ✅ Chosen target: Incentivized **pre-game sports market making** (top-liquidity leagues)

Rationale:
- Explicitly large, recurring liquidity incentive pools (documented monthly schedules).  
  _Source: https://docs.polymarket.com/market-makers/liquidity-rewards_
- Additional maker rebates funded by taker fees in eligible categories.  
  _Source: https://docs.polymarket.com/market-makers/maker-rebates_
- Repeated event structure = stronger fit for systematic feature engineering and meta-labeling.
- API surfaces let you rank/scan markets by spread, rewards, competitiveness, and volume.

Initial subfocus (lowest complexity):
- **Pre-game windows** only (avoid live microstructure complexity at first),
- high daily reward-rate events,
- strict max inventory and timeout/kill-switch rules.

---

## Step-by-Step BMAD Execution Plan (No Code)

### Phase 0 — Guardrails & Preconditions
1. Confirm geo-eligibility and legal scope for deployment region/IP.
2. Decide wallet model: Safe/Proxy if gasless path is required.
3. Define risk policy: max daily drawdown, max per-market inventory, kill-switch triggers.
4. Define “go/no-go” KPI gates before any scaling.

### Phase 1 — Market Universe & Opportunity Ranking
1. Pull sampling/reward-eligible markets continuously.
2. Rank by composite score: reward rate, spread, realized fill frequency proxy, volume, competitiveness.
3. Exclude markets with poor operational characteristics (insufficient depth, unstable timing windows).
4. Freeze first launch basket (small, homogeneous market set).

### Phase 2 — Execution Foundation (SDK-First)
1. Lock dependency policy: only official `polymarket-client-sdk` with `ws` + `clob` features.
2. Build capability acceptance tests (readiness checklist, no strategy alpha yet):
   - market WS stream health,
   - user WS auth stream health,
   - post/cancel/batch endpoints,
   - heartbeat continuity,
   - order/trade reconciliation.
3. Implement rate-limit-aware request scheduling and retry envelopes from official error matrix.

### Phase 3 — Baseline Market-Making Engine
1. Start with static two-sided quoting around midpoint with conservative spread.
2. Add inventory skew rules (shift bid/ask based on inventory imbalance).
3. Add GTD expiry rules around known event boundaries.
4. Add maker-only safeguards (post-only where appropriate).
5. Add forced cancel-all and cancel-by-market recovery playbooks.

### Phase 4 — López de Prado Research Overlay
1. Build event-based labeled dataset from your first basket:
   - event timestamps,
   - quote state / microstructure state,
   - external reference odds drift.
2. Generate triple-barrier labels for quote decisions.
3. Train primary signal (side/skew intent), then meta-model (trade/skip confidence).
4. Calibrate probabilities and map to position/quote size.
5. Introduce fractional-diff feature set and compare marginal utility.

### Phase 5 — Validation & Overfitting Defense
1. Use purged + embargo CV for all model tuning.
2. Add CPCV for distributional robustness (not one backtest path).
3. Evaluate by economic metrics, not just classifier metrics:
   - net spread capture,
   - rebate + reward contribution,
   - inventory carry cost,
   - tail loss episodes.
4. Reject models that improve paper metrics but worsen execution slippage/tail behavior.

### Phase 6 — Paper-to-Production Rollout
1. Dry run in read-only + shadow quoting mode.
2. Move to tiny notional production with strict loss caps.
3. Scale only if KPI gates are met for consecutive evaluation windows.
4. Operationalize alerting:
   - WS disconnect frequency,
   - heartbeat failures,
   - retry storms (425/429),
   - unexplained reconciliation gaps.

### Phase 7 — DigitalOcean Deployment (Hardening First)
1. Do not run long-term trading as `root`; create least-privilege service user.
2. Move secrets to vault/manager; do not leave plaintext secrets in shell history.
3. Add process supervision, restart policy, and health probes.
4. Separate execution, strategy, and reconciliation workers for blast-radius control.

---

## Requirement Traceability (User-Specified)

| Requirement | Covered by official source | Notes |
|---|---|---|
| Cargo features `["ws", "clob"]` | Yes | Rust SDK feature flags documented. |
| Market WS (orderbook + price updates) | Yes | Market channel events and endpoint documented. |
| User WS (authenticated orders/fills) | Yes | User channel auth and order/trade events documented. |
| Full CLOB trading (limit/market/cancel/batch) | Yes | POST `/order`, POST `/orders`, DELETE variants, order types GTC/GTD/FOK/FAK documented. |
| Gasless via Builder (if possible) | Yes | Builder + relayer flows documented (Safe/Proxy + headers/SDKs). |
| Clean async with tokio | Feasible | Rust SDK is async-first; architecture should use structured async task domains. |
| Reconnection, heartbeats, error handling | Yes | WS ping/pong, heartbeat endpoint, rate limits, and error codes documented. |

Key sources:  
- https://github.com/Polymarket/rs-clob-client  
- https://docs.polymarket.com/api-reference/wss/market  
- https://docs.polymarket.com/api-reference/wss/user  
- https://docs.polymarket.com/api-reference/trade/post-a-new-order  
- https://docs.polymarket.com/api-reference/trade/post-multiple-orders  
- https://docs.polymarket.com/api-reference/trade/cancel-single-order  
- https://docs.polymarket.com/api-reference/trade/cancel-multiple-orders  
- https://docs.polymarket.com/api-reference/trade/cancel-all-orders  
- https://docs.polymarket.com/api-reference/trade/cancel-orders-for-a-market  
- https://docs.polymarket.com/api-reference/trade/send-heartbeat  
- https://docs.polymarket.com/resources/error-codes  
- https://docs.polymarket.com/api-reference/rate-limits  
- https://docs.polymarket.com/trading/gasless  
- https://docs.polymarket.com/trading/orders/attribution

---

## Implementation Priorities (Pragmatic)

1. **First alpha source:** spread + rebates + liquidity rewards (not pure prediction).  
2. **First market class:** pre-game incentivized sports with strong liquidity/reward profile.  
3. **First quant upgrade:** meta-labeling on top of baseline quoting decisions.  
4. **First risk non-negotiables:** cancel-all kill switch, heartbeat watchdog, strict inventory caps.

---

## Research Conclusion

The most actionable path to early profitability on Polymarket with Rust is **execution-first, incentive-aware market making** in reward-eligible markets, then incremental López de Prado overlays for smarter participation and risk-adjusted sizing. This path matches your constraints (official Rust SDK only, no-code planning, WS+CLOB+builder coverage, reliability requirements) and minimizes initial model risk.

**Recommended next BMAD action:** convert this plan into epics/stories with explicit acceptance gates per phase and a weekly KPI review cadence.

---

**Research Completion Date:** 2026-04-04  
**Source Verification:** Completed using official Polymarket docs + open method references  
**Confidence Level:** High for platform capability mapping; Medium-High for strategy edge persistence (depends on execution quality and competition)
