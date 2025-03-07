use chrono::NaiveDate;
use history_model::HistoryEntry;
use log::debug;
use serde::{Deserialize, Serialize};
use std::error::Error;

pub struct CbrAPI {
    base_url: String,
    client: reqwest::Client,
    headers: reqwest::header::HeaderMap,
}

impl CbrAPI {
    pub fn new() -> Self {
        let mut reqwest_headers = reqwest::header::HeaderMap::new();
        reqwest_headers.insert(reqwest::header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36".parse().unwrap());

        return CbrAPI {
            base_url: "https://www.cbr.ru".to_string(),
            client: reqwest::Client::new(),
            headers: reqwest_headers,
        };
    }

    pub async fn get_ticker(&self, ticker: &str) -> Result<Vec<HistoryEntry>, Box<dyn Error>> {
        let code = self.map_ticker_to_code(ticker);
        let start_date = "01/01/2014";
        let end_date = chrono::Local::now().format("%d/%m/%Y").to_string();

        let url = format!(
            "{}/scripts/XML_dynamic.asp?date_req1={}&date_req2={}&VAL_NM_RQ={}",
            self.base_url, start_date, end_date, code
        );

        debug!("get_ticker | url: {}", url);

        let cbr_xml_str = self
            .client
            .get(&url)
            .headers(self.headers.clone())
            .send()
            .await?
            .text()
            .await?;

        let cbr_xml: CbrApiXML = quick_xml::de::from_str(&cbr_xml_str).unwrap();

        let history: Vec<HistoryEntry> = cbr_xml
            .record
            .iter()
            .map(|r| HistoryEntry {
                date: NaiveDate::parse_from_str(&r.date, "%d.%m.%Y").unwrap_or_default(),
                close: self.parse_cbr_float(&r.vunit_rate),
                low: self.parse_cbr_float(&r.vunit_rate),
                high: self.parse_cbr_float(&r.vunit_rate),
                volume: 0,
                facevalue: 1,
            })
            .collect();

        Ok(history)
    }

    fn map_ticker_to_code(&self, ticker: &str) -> String {
        match ticker {
            "usd" => "R01235".to_string(),
            "cny" => "R01375".to_string(),
            "eur" => "R01239".to_string(),
            _ => "R01235".to_string(), // default as usd
        }
    }

    fn parse_cbr_float(&self, float_str: &str) -> f64 {
        float_str.replace(",", ".").parse().unwrap_or_default()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CbrApiXML {
    #[serde(rename = "Record")]
    record: Vec<Record>,

    #[serde(rename = "@ID")]
    id: String,

    #[serde(rename = "@DateRange1")]
    date_range1: String,

    #[serde(rename = "@DateRange2")]
    date_range2: String,

    #[serde(rename = "@name")]
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Record {
    #[serde(rename = "Nominal")]
    nominal: String,

    #[serde(rename = "Value")]
    value: String,

    #[serde(rename = "VunitRate")]
    vunit_rate: String,

    #[serde(rename = "@Date")]
    date: String,

    #[serde(rename = "@Id")]
    id: String,
}
