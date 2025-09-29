use std::sync::Arc;
// Web dashboard using Warp
use warp::Filter;

use tokio::sync::Mutex;

// Shared state for live signals
#[derive(Clone, Debug, serde::Serialize)]
pub struct SignalInfo {
    pub coin: String,
    pub timeframe: String,
    pub signal: String,
    pub strength: i32,
    pub volume: f64,
    pub timestamp: String,
}

pub type SignalStore = Arc<Mutex<Vec<SignalInfo>>>;

pub async fn run_web_dashboard_with_signals(signal_store: SignalStore) {
    let dashboard = warp::path::end().map(|| {
        warp::reply::html(r#"
        <!DOCTYPE html>
        <html lang='en'>
        <head>
            <meta charset='UTF-8'>
            <meta name='viewport' content='width=device-width, initial-scale=1.0'>
            <title>AI Agent Dashboard</title>
            <style>
                body { font-family: 'Segoe UI', Arial, sans-serif; background: #181818; color: #f5f5f5; margin: 0; }
                header { background: #222; padding: 0.5rem; text-align: center; font-size: 1.2rem; color: #ffd700; }
                .container { max-width: 900px; margin: 1rem auto; padding: 1rem; background: #222; border-radius: 8px; box-shadow: 0 2px 8px #000a; }
                h2 { color: #ffd700; font-size: 1rem; margin-bottom: 0.5rem; }
                table { width: 100%; border-collapse: collapse; margin-top: 1rem; font-size: 0.85rem; }
                th, td { padding: 0.3rem 0.4rem; border-bottom: 1px solid #444; text-align: left; }
                th { background: #333; color: #ffd700; font-size: 0.9rem; }
                tr:hover { background: #333; }
                .buy { color: #00ff99; font-weight: bold; }
                .sell { color: #ff4d4d; font-weight: bold; }
                .strength-bar { display: inline-block; height: 10px; border-radius: 3px; vertical-align: middle; margin-right: 4px; }
                .strength-low { width: 20px; background: #ff4d4d; }
                .strength-med { width: 40px; background: #ffd700; }
                .strength-high { width: 60px; background: #00ff99; }
                td[title] { cursor: help; }
            </style>
            <script>
                async function fetchSignals() {
                    const res = await fetch('/api/signals');
                    const data = await res.json();
                    const tbody = document.getElementById('signals-body');
                    tbody.innerHTML = '';
                    for (const s of data) {
                        let barClass = 'strength-low';
                        if (s.strength >= 80) barClass = 'strength-high';
                        else if (s.strength >= 60) barClass = 'strength-med';
                        tbody.innerHTML += `<tr>
                            <td title='${s.coin} perpetual'>${s.coin}</td>
                            <td title='${s.timeframe}'>${s.timeframe}</td>
                            <td class='${s.signal}' title='${s.signal} signal'>${s.signal.charAt(0).toUpperCase() + s.signal.slice(1)}</td>
                            <td title='${s.strength}/140 points'><span class='strength-bar ${barClass}'></span>${s.strength}</td>
                            <td title='Volume'>${Math.round(s.volume/1000)}k</td>
                            <td title='Signal time'>${s.timestamp}</td>
                        </tr>`;
                    }
                }
                setInterval(fetchSignals, 5000);
                window.onload = fetchSignals;
            </script>
        </head>
        <body>
            <header>AI Agent Dashboard</header>
            <div class='container'>
                <h2>Summary</h2>
                <p>Monitoring <b>146</b> perpetual coins across 5 timeframes (5m, 15m, 1h, 4h, 1d) using EMA 12/26. Signals are sent to Telegram in real-time.</p>
                <h2>Latest Signals</h2>
                <table>
                    <thead>
                        <tr>
                            <th>Coin</th>
                            <th>TF</th>
                            <th>Signal</th>
                            <th>Strength</th>
                            <th>Vol</th>
                            <th>Time</th>
                        </tr>
                    </thead>
                    <tbody id='signals-body'>
                        <!-- Live signals will be injected here -->
                    </tbody>
                </table>
            </div>
        </body>
        </html>
        "#)
    });

    let api = warp::path("api")
        .and(warp::path("signals"))
        .and(warp::get())
        .and_then(move || {
            let signal_store = signal_store.clone();
            async move {
                let signals = signal_store.lock().await;
                Ok::<_, warp::Rejection>(warp::reply::json(&*signals))
            }
        });

    let routes = dashboard.or(api);
    warp::serve(routes).run(([0, 0, 0, 0], 8080)).await;
}
}
