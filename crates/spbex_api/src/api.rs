use history_model::HistoryEntry;
use itertools::izip;
use log::debug;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;

pub struct SpbexAPI {
    base_url: String,
    client: reqwest::Client,
    headers: reqwest::header::HeaderMap,
}

impl SpbexAPI {
    pub fn new() -> Self {
        let mut reqwest_headers = reqwest::header::HeaderMap::new();
        reqwest_headers.insert(reqwest::header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36".parse().unwrap());

        return SpbexAPI {
            base_url: "https://investcab.ru/api".to_string(),
            client: reqwest::Client::new(),
            headers: reqwest_headers,
        };
    }

    pub async fn get_ticker(&self, ticker: &str) -> Result<Vec<HistoryEntry>, Box<dyn Error>> {
        let timerange = self.get_time_range();
        let url = format!(
            "{}/chistory?symbol={}&resolution={}&from={}&to={}",
            self.base_url, ticker, "D", timerange.start, timerange.end
        );

        debug!("get_ticker | url: {}", url);

        let spbex_json_str = self
            .client
            .get(&url)
            .headers(self.headers.clone())
            .send()
            .await?
            .text()
            .await?
            .replace("\\", "");

        let spbex_json: SpbexHistoryJSON =
            serde_json::from_str(&spbex_json_str[1..spbex_json_str.len() - 1])?;

        if spbex_json.t.len() == 0 {
            return Err(Box::new(CustomError::NotFound));
        }

        let history = izip!(&spbex_json.t, &spbex_json.h, &spbex_json.l, &spbex_json.c)
            .map(|(t, h, l, c)| HistoryEntry {
                date: chrono::DateTime::from_timestamp(*t, 0)
                    .unwrap_or_default()
                    .date_naive(),
                close: *c,
                high: *h,
                low: *l,
                volume: 0,
                facevalue: 1,
            })
            .collect();

        Ok(history)
    }

    fn get_time_range(&self) -> TimeRange {
        TimeRange {
            start: 0,
            end: chrono::Local::now().timestamp(),
        }
    }
}

struct TimeRange {
    start: i64,
    end: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpbexHistoryJSON {
    pub t: Vec<i64>,
    pub o: Vec<f64>,
    pub h: Vec<f64>,
    pub l: Vec<f64>,
    pub c: Vec<f64>,
    pub s: String,
}

#[derive(Debug)]
pub enum CustomError {
    NotFound,
}

impl fmt::Display for CustomError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CustomError::NotFound => write!(f, "Not found"),
        }
    }
}

impl Error for CustomError {}
