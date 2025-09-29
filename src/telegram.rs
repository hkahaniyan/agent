// Telegram bot integration
pub struct TelegramBot {
    pub token: String,
    pub chat_id: String,
}

impl TelegramBot {
    pub fn new(token: String, chat_id: String) -> Self {
        Self { token, chat_id }
    }
    pub async fn send_signal(&self, market: &str, timeframe: &str, signal: &str) -> Result<(), reqwest::Error> {
        let message = format!("{} {}: {} signal", market, timeframe, signal);
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.token);
        let params = [
            ("chat_id", self.chat_id.as_str()),
            ("text", &message),
        ];
        let client = reqwest::Client::new();
        client.post(&url)
            .form(&params)
            .send()
            .await?;
        Ok(())
    }
}
