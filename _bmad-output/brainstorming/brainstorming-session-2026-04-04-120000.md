---
stepsCompleted: [1, 2, 3, 4]
inputDocuments:
  - /Users/sang/polymarket-ai/_bmad-output/planning-artifacts/research/domain-polymarket-rust-clob-profit-research-2026-04-04.md
  - /Users/sang/polymarket-ai/_bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md
session_topic: 'Profitable Rust Polymarket trading system (official polymarket-client-sdk only)'
session_goals: 'Brainstorm a comprehensive, profitable, resilient, and research-rigorous Rust Polymarket trading system using insights from both research documents.'
selected_approach: 'alpha-filter-pass'
techniques_used:
  - Alpha Filter Pass
  - Constraint-Based Pruning
  - Real-Alpha Prioritization
ideas_generated:
  - '30 real-alpha ideas (latency-independent)'
context_file: ''
technique_execution_complete: true
session_active: false
workflow_completed: true
---

# Brainstorming Session Results (Alpha-Filtered, No HFT Dependency)

**Facilitator:** Sang  
**Date:** 2026-04-04

## Session Overview

**Topic:** Profitable Rust Polymarket trading system, using only the official Rust SDK and aligned with López de Prado quant methods.

**Refinement Goal (your constraint):**
- Remove ideas that require speed/latency superiority (top VPS, ultra-low jitter, HFT-like infra).
- Keep ideas where edge comes from **real alpha**: incentives, market selection, signal quality, and rigorous validation.

---

## Technique Selection

**Approach:** Alpha Filter Pass (post-brainstorm refinement)

**Filter rules used:**
1. Reject any idea that depends on queue-racing or reaction-speed dominance.
2. Reject ideas where success is mostly infra advantage, not information/economic edge.
3. Keep ideas that can work on normal VPS + stable internet if execution is disciplined.

---

## Technique Execution Results

### Selected Real-Alpha Ideas (30)

### A) Incentive and Market-Economics Alpha (8)
1. **[#11] Rebate Density Scanner** — rank markets by maker rebates per unit risk.
2. **[#12] Incentive-Hours Overlap** — trade when reward + participation overlap is strongest.
3. **[#13] Reward-Per-Risk Router** — allocate by $\frac{\text{spread} + \text{rebates} + \text{rewards}}{\text{VaR}}$.
4. **[#14] Fee-Curve Sweet Spot** — target price regions with better fee-adjusted economics.
5. **[#15] Two-Sided Score Protector** — maintain reward-eligible two-sided presence.
6. **[#17] Rebate Recovery Windows** — controlled economics recovery after adverse episodes.
7. **[#18] Incentive Cliff Alarm** — detect reward regime changes and rerank markets fast.
8. **[#20] Reward Budget Hedger** — reserve reward share for drawdown buffer and R&D.

### B) Market Selection and Regime Alpha (8)
9. **[#21] Top-Decile Sports Core** — focus on durable high-liquidity event sets.
10. **[#22] Weather Shock Satellite** — diversify into orthogonal liquid domains.
11. **[#24] News-Catalyst Exclusion Zones** — avoid uncertainty windows without modeled edge.
12. **[#25] Liquidity Half-Life Filter** — select markets where depth persists.
13. **[#26] Spread Persistence Ranker** — prioritize markets with monetizable spread duration.
14. **[#28] Volume Regime Buckets** — different policies by market activity regime.
15. **[#29] Resolution Risk Ladder** — cap capital for complex-resolution markets.
16. **[#30] Overnight Gap Sentinel** — reduce exposure in participation drought windows.

### C) Statistical/Model Alpha (14)
17. **[#51] Event-Time Bars** — sample on information arrival, not clock-time.
18. **[#52] CUSUM Trigger Matrix** — regime-specific event triggers.
19. **[#53] Triple-Barrier per Regime** — adaptive labeling by volatility/event type.
20. **[#54] Meta-Label Confidence Bands** — quote full/reduced/skip by confidence.
21. **[#56] Sequential Bootstrap Weighting** — de-emphasize redundant observations.
22. **[#59] Confidence-to-Quote Curve** — smooth confidence-to-size/spread mapping.
23. **[#61] Purged Walk-Forward Stack** — leakage-safe temporal validation.
24. **[#62] CPCV Scenario Library** — evaluate distribution of paths, not one backtest.
25. **[#63] DSR Promotion Gate** — require robust risk-adjusted performance before promotion.
26. **[#64] MinTRL Launch Rule** — enforce track-record length before scaling.
27. **[#65] PBO Dashboard** — penalize overfit candidates explicitly.
28. **[#68] Label Leakage Linter** — continuous leakage checks in research pipeline.
29. **[#69] Counterfactual Replay** — measure sensitivity to favorable execution luck.
30. **[#70] Stop-Research Criteria** — kill weak branches early; reduce false discovery.

### Removed (Speed/Latency-Dependent) Themes

- Queue-race / microsecond-reactive ideas: **#1, #2, #4, #8, #9, #10**.
- Reaction-time-sensitive execution control: **#6, #7, #32, #48, #49**.
- Infra-first mechanics that are useful but not “real alpha” by themselves: most of **#41-50**.

---

## Idea Organization and Prioritization

### Thematic Clusters
1. **Economic Edge Layer** (Ideas #11-20 subset)
2. **Market/Regime Selection Layer** (Ideas #21-30 subset)
3. **Model Integrity Layer** (Ideas #51-70 subset)

### Prioritized Top Opportunities

#### Priority 1 — Incentive Economics Engine
- Combines: **#11, #13, #15, #18, #20**.
- Why this is real alpha: it monetizes structural platform economics, not speed.

#### Priority 2 — Market Regime Scoring Engine
- Combines: **#21, #25, #26, #28, #30**.
- Why this is real alpha: it selects where edge is durable before execution starts.

#### Priority 3 — López de Prado Validation Gatekeeper
- Combines: **#52, #53, #54, #61, #62, #63, #65, #68**.
- Why this is real alpha: it protects against overfitting and keeps only robust signal.

### Quick Wins (First 2 Weeks)
- Build market ranking from **#13 + #25 + #26**.
- Define reward regime alerts from **#18**.
- Set promotion policy with **DSR + MinTRL** from **#63 + #64**.

---

## Action Planning (BMAD-Aligned)

### 30-Day Plan
1. **Business (B):**
   - Implement opportunity scorecard: incentive yield, spread persistence, regime stability.
   - Set explicit capital split between core universe (#21) and satellite universe (#22).

2. **Model (M):**
   - Build event-label pipeline using #51, #52, #53, #54.
   - Add leakage and overfit controls (#61, #62, #65, #68).

3. **Architecture (A):**
   - Keep infra simple and robust (no latency-arb assumptions).
   - Focus telemetry on score quality and validation gates, not reaction-time arms race.

4. **Delivery (D):**
   - Run paper/shadow with promotion gate: DSR + MinTRL + PBO constraints.

### 60-Day Plan
- Promote only the highest-stability market buckets from regime scoring.
- Retrain/retune only through CPCV + purged walk-forward evidence.
- Use #70 stop-research rule to eliminate weak branches fast.

### 90-Day Plan
- Scale capital gradually by validated buckets, not by aggregate PnL spikes.
- Keep alpha concentration on economic edge + regime selection + robust modeling.

---

## Session Summary and Insights

### Key Achievements
- Reduced the brainstorm from **100 mixed ideas** to **30 real-alpha ideas**.
- Removed speed/latency-dependent concepts inconsistent with your infra constraints.
- Re-centered roadmap around robust, non-HFT edge sources.

### Most Important Insight
Your best edge is a **three-layer real-alpha stack**:
1. structural incentive economics,
2. disciplined market/regime selection,
3. strict anti-overfitting model governance.

### Recommended Immediate Next Step
Create the next artifact as a sprint-ready implementation brief for these 3 tracks:
- Incentive Economics Engine,
- Market Regime Scoring Engine,
- López de Prado Validation Gatekeeper.

