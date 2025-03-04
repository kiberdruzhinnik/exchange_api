use history_model::HistoryEntry;
use std::error::Error;

pub struct CbrAPI {
    base_url: String,
}

impl CbrAPI {
    pub fn new() -> Self {
        return CbrAPI {
            base_url: "https://www.cbr.ru".to_string(),
        };
    }

    pub async fn get_ticker(&self, ticker: &str) -> Result<Vec<HistoryEntry>, Box<dyn Error>> {
        return Ok(vec![] as Vec<HistoryEntry>);
    }
}
