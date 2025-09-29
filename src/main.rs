mod delta;
mod ema;
mod telegram;
mod web;

use simple_logger::SimpleLogger;
use log::{info, error};
use tokio::time::{interval, Duration};
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    SimpleLogger::new().init().unwrap();
    info!("AI Agent started");
    // TODO: Load API keys from environment or config
    let api_key = std::env::var("DELTA_API_KEY").expect("DELTA_API_KEY not set");
    let api_secret = std::env::var("DELTA_API_SECRET").expect("DELTA_API_SECRET not set");
    let delta_client = delta::DeltaClient::new(api_key, api_secret);
    let markets = match delta_client.fetch_perpetual_markets().await {
        Ok(m) => m,
        Err(e) => {
            error!("Failed to fetch perpetual coins: {}", e);
            return;
        }
    };

    let timeframes = vec![("5m", 5), ("15m", 15), ("1h", 60), ("4h", 240), ("1d", 1440)];
    let telegram_token = std::env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not set");
    let telegram_chat_id = std::env::var("TELEGRAM_CHAT_ID").expect("TELEGRAM_CHAT_ID not set");
    let telegram_bot = telegram::TelegramBot::new(telegram_token, telegram_chat_id);

    use std::collections::HashMap;
    use tokio::sync::Mutex;
    use std::sync::Arc;
    // Store trades as (price, volume, timestamp)
    let trade_data: Arc<Mutex<HashMap<String, Vec<(f64, f64, u64)>>>> = Arc::new(Mutex::new(HashMap::new()));

    let symbols = markets.clone();
    let trade_data_clone = trade_data.clone();
    tokio::spawn(async move {
        delta_client.stream_realtime_prices(symbols, move |symbol, price, volume| {
            let mut data = futures::executor::block_on(trade_data_clone.lock());
            let ts = chrono::Utc::now().timestamp() as u64;
            let entry = data.entry(symbol.clone()).or_insert_with(Vec::new);
            entry.push((price, volume, ts));
            if entry.len() > 500 { entry.remove(0); }
        }).await;
    });

    // Shared signal store for dashboard/API
    let signal_store: web::SignalStore = Arc::new(Mutex::new(Vec::new()));
    let trade_data_signal = trade_data.clone();
    let telegram_bot_signal = telegram_bot.clone();
    let signal_store_signal = signal_store.clone();
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(300)); // 5 minutes
        loop {
            interval.tick().await;
            let data = trade_data_signal.lock().await;
            let mut new_signals = Vec::new();
            for (symbol, trades) in data.iter() {
                let mut tf_signals = Vec::new();
                let mut tf_volumes = Vec::new();
                for (tf_name, tf_minutes) in &[("5m", 5), ("15m", 15), ("1h", 60), ("4h", 240), ("1d", 1440)] {
                    let candles = delta::aggregate_candles(trades, tf_minutes * 60);
                    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
                    let total_volume: f64 = candles.iter().map(|c| c.volume).sum();
                    tf_volumes.push(total_volume);
                    tf_signals.push(ema::detect_ema_signals(&closes));
                }
                let mut buy_count = 0;
                let mut sell_count = 0;
                let mut points = 0;
                for sig in &tf_signals {
                    match sig {
                        Some("buy") => { buy_count += 1; points += 20; },
                        Some("sell") => { sell_count += 1; points += 20; },
                        _ => {}
                    }
                }
                let max_volume = tf_volumes.iter().cloned().fold(0.0, f64::max);
                let volume = tf_volumes.get(0).cloned().unwrap_or(0.0); // 5m volume
                let volume_boost = if max_volume > 0.0 { ((volume / max_volume) * 20.0).round() as i32 } else { 0 };
                let candles_5m = delta::aggregate_candles(trades, 5 * 60);
                let closes_5m: Vec<f64> = candles_5m.iter().map(|c| c.close).collect();
                let macd_signal = ema::detect_macd_crossover(&closes_5m);
                let mut macd_boost = 0;
                let direction = if buy_count >= 2 && points >= 40 {
                    Some("buy")
                } else if sell_count >= 2 && points >= 40 {
                    Some("sell")
                } else {
                    None
                };
                if let (Some(macd), Some(dir)) = (macd_signal, direction.as_ref()) {
                    if macd == *dir {
                        macd_boost = 20;
                    }
                }
                let strength = points + volume_boost + macd_boost; // out of 140
                if let Some(dir) = direction {
                    let ts = chrono::Utc::now().format("%H:%M:%S").to_string();
                    info!("{}: {} signal (strength: {} points, volume boost: {}, macd boost: {})", symbol, dir, strength, volume_boost, macd_boost);
                    let _ = telegram_bot_signal.send_signal(symbol, "5m", &format!("{} (strength: {} points, volume boost: {}, macd boost: {})", dir, strength, volume_boost, macd_boost)).await;
                    // Push to signal store for dashboard/API
                    new_signals.push(web::SignalInfo {
                        coin: symbol.clone(),
                        timeframe: "5m".to_string(),
                        signal: dir.to_string(),
                        strength,
                        volume,
                        timestamp: ts,
                    });
                }
            }
            // Update shared signal store
            let mut store = signal_store_signal.lock().unwrap();
            *store = new_signals;
        }
    });
    // Start web dashboard server with live signals
    web::run_web_dashboard_with_signals(signal_store).await;
}
