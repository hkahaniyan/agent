use tokio_tungstenite::tungstenite::Message;
use futures_util::{StreamExt, SinkExt};
// OHLCV candle struct for chart matching
#[derive(Clone, Debug)]
pub struct Candle {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub timestamp: u64, // Unix timestamp in seconds
}

// Aggregates trades into OHLCV candles for a given timeframe (in seconds)
pub fn aggregate_candles(trades: &[(f64, f64, u64)], timeframe_sec: u64) -> Vec<Candle> {
    use std::collections::BTreeMap;
    let mut buckets: BTreeMap<u64, Vec<(f64, f64)>> = BTreeMap::new();
    for &(price, volume, ts) in trades {
        let bucket = ts - (ts % timeframe_sec);
        buckets.entry(bucket).or_default().push((price, volume));
    }
    let mut candles = Vec::new();
    for (&bucket, entries) in buckets.iter() {
        let open = entries.first().map(|x| x.0).unwrap_or(0.0);
        let close = entries.last().map(|x| x.0).unwrap_or(0.0);
        let high = entries.iter().map(|x| x.0).fold(f64::MIN, f64::max);
        let low = entries.iter().map(|x| x.0).fold(f64::MAX, f64::min);
        let volume = entries.iter().map(|x| x.1).sum();
        candles.push(Candle { open, high, low, close, volume, timestamp: bucket });
    }
    candles
}
// Handles Delta Exchange API integration
pub struct DeltaClient {
    pub api_key: String,
    pub api_secret: String,
}

impl DeltaClient {
    use tokio_tungstenite::tungstenite::Message;
    // Connects to Delta Exchange WebSocket for real-time price updates (5m base timeframe)
    pub async fn stream_realtime_prices<F>(&self, symbols: Vec<String>, mut on_price: F)
    where
        F: FnMut(String, f64, f64) + Send + 'static,
    {
        use tokio_tungstenite::connect_async;
        use futures_util::{StreamExt, SinkExt};
        use tokio::time::{sleep, Duration, Instant};
        use std::collections::VecDeque;
        let ws_url = "wss://socket.delta.exchange";
        let mut backoff = 1;
        let max_backoff = 32;
        let api_limit = 60; // max 60 events per minute
        let mut event_times: VecDeque<Instant> = VecDeque::new();
        loop {
            match connect_async(ws_url).await {
                Ok((ws_stream, _)) => {
                    let (mut write, mut read) = ws_stream.split();
                    for symbol in &symbols {
                        let sub_msg = serde_json::json!({
                            "type": "subscribe",
                            "channels": [{"name": "v2/ticker", "symbols": [symbol]}]
                        });
                        let _ = write.send(tokio_tungstenite::tungstenite::Message::Text(sub_msg.to_string().into())).await;
                    }
                    while let Some(msg) = read.next().await {
                        if let Ok(Message::Text(txt)) = msg {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&txt) {
                                if let Some(data) = json.get("data") {
                                    if let (Some(symbol), Some(price), Some(volume)) = (data.get("symbol"), data.get("mark_price"), data.get("volume_24h")) {
                                        if let (Some(symbol), Some(price), Some(volume)) = (symbol.as_str(), price.as_f64(), volume.as_f64()) {
                                            // API rate limiting
                                            let now = Instant::now();
                                            event_times.push_back(now);
                                            while let Some(&front) = event_times.front() {
                                                if now.duration_since(front) > Duration::from_secs(60) {
                                                    event_times.pop_front();
                                                } else {
                                                    break;
                                                }
                                            }
                                            if event_times.len() > api_limit {
                                                // Too many events, skip this one
                                                continue;
                                            }
                                            on_price(symbol.to_string(), price, volume);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // If we exit the loop, connection dropped
                    eprintln!("WebSocket disconnected, reconnecting in {}s...", backoff);
                    sleep(Duration::from_secs(backoff)).await;
                    backoff = (backoff * 2).min(max_backoff);
                }
                Err(e) => {
                    eprintln!("WebSocket connection error: {}. Retrying in {}s...", e, backoff);
                    sleep(Duration::from_secs(backoff)).await;
                    backoff = (backoff * 2).min(max_backoff);
                }
            }
        }
    }
    pub fn new(api_key: String, api_secret: String) -> Self {
        Self { api_key, api_secret }
    }
    // Fetch all perpetual coins from Delta Exchange
    pub async fn fetch_perpetual_markets(&self) -> Result<Vec<String>, reqwest::Error> {
        let url = "https://api.delta.exchange/v2/products";
        let client = reqwest::Client::new();
        let resp = client.get(url)
            .header("api-key", &self.api_key)
            .send()
            .await?;
        let json: serde_json::Value = resp.json().await?;
        let mut markets = Vec::new();
        if let Some(products) = json.get("result").and_then(|r| r.as_array()) {
            for prod in products {
                if let Some(market_type) = prod.get("contract_type") {
                    if market_type == "perpetual" {
                        if let Some(symbol) = prod.get("symbol").and_then(|s| s.as_str()) {
                            markets.push(symbol.to_string());
                        }
                    }
                }
            }
        }
        Ok(markets)
    }
}
