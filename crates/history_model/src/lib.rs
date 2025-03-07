use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub date: NaiveDate,
    pub close: f64,
    pub high: f64,
    pub low: f64,
    pub volume: i64,
    pub facevalue: i64,
}
