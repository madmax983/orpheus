# 🔭 Vantage: Spec for Volatile Markets Backtesting

## 👤 User Story
"As a Trader, I want to backtest against volatile markets..."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus is a music creation tool. Traders lack a way to ingest historical market data and map it to audio parameters to sonify market volatility. By adding data ingestion and sonification capabilities, we open Orpheus to financial analysts and algorithmic traders. Complexity is a cost; utility is a revenue. A tool that helps traders identify patterns through audio is a high-utility feature.

## 🎯 Metric Definition
- **Success** = Users can import historical market data (CSV format) and map data columns to audio parameters, with the engine smoothly handling missing data (NaN) without crashing.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Orpheus only generates sound from internal patterns or samples. It has no capability to ingest external time-series data.
- **Competitors:** Most trading platforms rely on visual charts. Some experimental tools offer data sonification, but lack a robust live-coding environment to manipulate the data mappings on the fly.
- **The Gap:** Orpheus needs a robust data ingestion layer that can read CSV files, handle missing or malformed data gracefully, and expose this data as manipulatable patterns.

## ✅ Acceptance Criteria
- Must handle NaN data without panicking.
- Must output a CSV report.

## 🚫 Out of Scope
- Real-time execution (Phase 2).
