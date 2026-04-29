# 🔭 Vantage: Spec for Volatile Markets Backtesting

## 👤 User Story
"As a Trader, I want to backtest against volatile markets, so that I can validate that my strategy can handle rapid price fluctuations and wide bid-ask spreads without taking excessive losses."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus' backtesting capabilities assume clean, sequential data with consistent latency and narrow spreads. While this is sufficient for basic strategy testing, it completely ignores the reality of flash crashes, market gaps, and extreme volatility. Without a way to inject volatility, NaN data, and missing ticks into the backtest data streams, traders deploy strategies into live markets blind to their true risk. Complexity is a cost; utility is a revenue. Providing realistic, volatile market simulations is critical utility for anyone risking real capital, transforming the backtester from a toy into a professional-grade risk management tool.

## 🎯 Metric Definition
- **Success** = Users can configure a volatility profile (e.g., `volatility(level=high)`) on a backtest run that injects missing data, sudden gaps (up to 5% price jumps), and randomized spread widening into the historical tick stream. The strategy under test must process this data in under 5ms per tick without panicking on NaN values or missing fields.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Backtests run on pristine datasets. There is no mechanism to simulate network jitter, dropped ticks, or extreme volatility.
- **Competitors (QuantConnect, TradeStation):** Professional platforms offer robust slippage, latency, and volatility simulation.
- **The Gap:** Orpheus needs a data mutation layer between the historical data loader and the backtest engine to introduce configurable "dirty" data and volatile market conditions.

## ✅ Acceptance Criteria
- Must introduce a volatility profile configuration for backtesting sessions.
- Must handle NaN data without panicking.
- Must simulate sudden price gaps and widened spreads based on the volatility profile.
- Must output a CSV report detailing strategy performance, including specific metrics on drawdowns during volatile periods.
- Must ensure that injecting volatility does not increase backtest execution time by more than 15%.

## 🚫 Out of Scope
- Real-time execution (Phase 2).
- Advanced order book (L2) dynamics simulation (e.g., modeling order queue positions during a flash crash). Phase 1 focuses on L1 tick data volatility.
