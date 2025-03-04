use history_model::HistoryEntry;
use std::error::Error;

pub struct SpbexAPI {
    base_url: String,
}

impl SpbexAPI {
    pub fn new() -> Self {
        return SpbexAPI {
            base_url: "https://investcab.ru/api".to_string(),
        };
    }

    pub async fn get_ticker(&self, ticker: &str) -> Result<Vec<HistoryEntry>, Box<dyn Error>> {
        return Ok(vec![] as Vec<HistoryEntry>);
    }
}
