// EMA calculation logic
pub fn calculate_ema(prices: &[f64], period: usize) -> Vec<f64> {
    let mut ema = Vec::new();
    let k = 2.0 / (period as f64 + 1.0);
    let mut prev_ema = prices[0];
    ema.push(prev_ema);
    for &price in prices.iter().skip(1) {
        let new_ema = price * k + prev_ema * (1.0 - k);
        ema.push(new_ema);
        prev_ema = new_ema;
    }
    ema
}

// Detects buy/sell signals based on EMA 12/26 crossover
pub fn detect_ema_signals(prices: &[f64]) -> Option<&'static str> {
    if prices.len() < 26 {
        return None;
    }
    let ema12 = calculate_ema(prices, 12);
    let ema26 = calculate_ema(prices, 26);
    let last_ema12 = ema12.last()?;
    let last_ema26 = ema26.last()?;
    let prev_ema12 = ema12.get(ema12.len().saturating_sub(2))?;
    let prev_ema26 = ema26.get(ema26.len().saturating_sub(2))?;
    if prev_ema12 < prev_ema26 && last_ema12 > last_ema26 {
        Some("buy")
    } else if prev_ema12 > prev_ema26 && last_ema12 < last_ema26 {
        Some("sell")
    } else {
        None
    }
}

// Points-based signal logic across 5 timeframes
pub fn points_based_signal(timeframe_signals: &[Option<&str>]) -> Option<&'static str> {
    // 60 points threshold, 5 timeframes, 12 points each
    let points_per_tf = 12;
    let mut buy_points = 0;
    let mut sell_points = 0;
    for signal in timeframe_signals {
        match signal {
            Some("buy") => buy_points += points_per_tf,
            Some("sell") => sell_points += points_per_tf,
            _ => {}
        }
    }
    if buy_points >= 60 {
        Some("buy")
    } else if sell_points >= 60 {
        Some("sell")
    } else {
        None
    }
}

// MACD crossover detection
pub fn detect_macd_crossover(prices: &[f64]) -> Option<&'static str> {
    if prices.len() < 35 {
        return None;
    }
    let ema12 = calculate_ema(prices, 12);
    let ema26 = calculate_ema(prices, 26);
    let macd: Vec<f64> = ema12.iter().zip(ema26.iter()).map(|(e12, e26)| e12 - e26).collect();
    let signal = calculate_ema(&macd, 9);
    let last_macd = macd.last()?;
    let last_signal = signal.last()?;
    let prev_macd = macd.get(macd.len().saturating_sub(2))?;
    let prev_signal = signal.get(signal.len().saturating_sub(2))?;
    if prev_macd < prev_signal && last_macd > last_signal {
        Some("buy")
    } else if prev_macd > prev_signal && last_macd < last_signal {
        Some("sell")
    } else {
        None
    }
}
