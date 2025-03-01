use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::fmt;
extern crate env_logger;
#[macro_use]
extern crate log;
use serde_json::json;
use std::error::Error;
use tower_http::trace::TraceLayer;

use axum::{extract::Path, routing::get, Json, Router};

const PAGE_SIZE: i64 = 100;
const MOEX_BASE_API_URL: &str = "https://iss.moex.com";

static MOEX_API: std::sync::LazyLock<MoexAPI> = std::sync::LazyLock::new(|| MoexAPI::new());

#[derive(Debug, Serialize, Deserialize)]
struct MoexSecurityParameters {
    board: String,
    market: String,
    engine: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MoexSecurityParametersJSON {
    boards: Boards,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Boards {
    columns: Vec<String>,
    data: Vec<(String, String, String, i64)>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MoexHistoryJSON {
    history: History,
    #[serde(rename = "history.cursor")]
    history_cursor: HistoryCursorJSON,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct History {
    columns: Vec<String>,
    data: Vec<Vec<serde_json::Value>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryCursorJSON {
    columns: Vec<String>,
    data: Vec<(i64, i64, i64)>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MoexPriceJSON {
    marketdata: Marketdata,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Marketdata {
    columns: Vec<String>,
    data: Vec<Vec<serde_json::Value>>,
}

struct MoexAPI {
    base_url: &'static str,
    client: reqwest::Client,
}

impl MoexAPI {
    fn new() -> Self {
        return MoexAPI {
            base_url: MOEX_BASE_API_URL,
            client: reqwest::Client::new(),
        };
    }

    async fn get_security_parameters(
        &self,
        ticker: &str,
    ) -> Result<MoexSecurityParameters, Box<dyn Error>> {
        let url = format!(
            "{}/iss/securities/{}.json?iss.only=boards&iss.meta=off&boards.columns=boardid,market,engine,is_primary",
            self.base_url, ticker
        );

        debug!("get_security_parameters url: {}", url);

        let moex_json = self
            .client
            .get(&url)
            .send()
            .await?
            .json::<MoexSecurityParametersJSON>()
            .await?;

        for entry in moex_json.boards.data {
            if entry.3 == 1 {
                return Ok(MoexSecurityParameters {
                    board: entry.0,
                    market: entry.1,
                    engine: entry.2,
                });
            }
        }

        Err(Box::new(CustomError::NotFound))
    }

    async fn get_regular_ticker(&self, ticker: &str) -> Result<Vec<HistoryEntry>, Box<dyn Error>> {
        let params = self.get_security_parameters(&ticker).await?;
        let mut total = PAGE_SIZE;
        let mut offset: i64 = 0;
        let mut history = Vec::new();
        while offset < total {
            let entry_history = self
                .get_security_history_offset(&ticker, &params, offset)
                .await?;
            total = entry_history.meta.total;
            offset += entry_history.meta.page_size;
            history.extend(entry_history.history);
        }

        if let Ok(mut current_price) = self.get_security_current_price(&ticker, &params).await {
            current_price.facevalue = history.last().unwrap().facevalue;
            history.push(current_price);
        }

        Ok(history)
    }

    async fn get_security_current_price(
        &self,
        ticker: &str,
        params: &MoexSecurityParameters,
    ) -> Result<HistoryEntry, Box<dyn Error>> {
        let url = format!(
            "{}/iss/engines/{}/markets/{}/securities/{}.json?iss.meta=off&iss.only=marketdata&marketdata.columns=BOARDID,LAST,HIGH,LOW,VOLTODAY",
            self.base_url, params.engine, params.market, ticker
        );

        debug!("get_security_current_price url: {}", url);

        let json = self
            .client
            .get(url)
            .send()
            .await?
            .json::<MoexPriceJSON>()
            .await?;

        for entry in json.marketdata.data {
            if entry[0] != params.board {
                continue;
            }
            
            let close = entry[1].as_f64().unwrap_or_default();
            let high = entry[2].as_f64().unwrap_or_default();
            let low = entry[3].as_f64().unwrap_or_default();
            let volume = entry[4].as_i64().unwrap_or_default();
            let facevalue = 1;

            if close == 0.0 || high == 0.0 || low == 0.0 || volume == 0 {
                return Err(Box::new(CustomError::NoData));
            }

            return Ok(HistoryEntry {
                date: chrono::Local::now().date_naive(),
                close,
                high,
                low,
                volume,
                facevalue,
            });
        }
        Err(Box::new(CustomError::NotFound))
    }

    async fn get_security_history_offset(
        &self,
        ticker: &str,
        params: &MoexSecurityParameters,
        offset: i64,
    ) -> Result<HistoryEntriesMoexMeta, Box<dyn Error>> {
        let url = format!(
                    "{}/iss/history/engines/{}/markets/{}/boards/{}/securities/{}.json?iss.meta=off&start={}&history.columns=TRADEDATE,CLOSE,HIGH,LOW,VOLUME,FACEVALUE",
                    self.base_url, params.engine, params.market, params.board, ticker, offset
                );

        debug!("get_security_history_offset url: {}", url);

        let json = self
            .client
            .get(url)
            .send()
            .await?
            .json::<MoexHistoryJSON>()
            .await?;

        let meta = HistoryCursor {
            offset: json.history_cursor.data[0].0,
            total: json.history_cursor.data[0].1,
            page_size: json.history_cursor.data[0].2,
        };

        let mut history = Vec::new();
        for entry in json.history.data {
            let date = NaiveDate::parse_from_str(entry[0].as_str().unwrap_or_default(), "%Y-%m-%d")?;
            // handle obligations
            let facevalue = match entry.len() {
                6 => entry[5].as_i64().unwrap_or(1),
                _ => 1,
            };

            history.push(HistoryEntry {
                date,
                close: entry[1].as_f64().unwrap_or_default(),
                high: entry[2].as_f64().unwrap_or_default(),
                low: entry[3].as_f64().unwrap_or_default(),
                volume: entry[4].as_i64().unwrap_or_default(),
                facevalue,
            })
        }

        Ok(HistoryEntriesMoexMeta { history, meta })
    }
}

#[derive(Debug)]
enum CustomError {
    NotFound,
    NoData,
}

impl fmt::Display for CustomError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CustomError::NotFound => write!(f, "Not found"),
            CustomError::NoData => write!(f, "No data"),
        }
    }
}

impl Error for CustomError {}

#[derive(Debug, Serialize, Deserialize)]
struct HistoryCursor {
    offset: i64,
    total: i64,
    page_size: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct HistoryEntriesMoexMeta {
    history: Vec<HistoryEntry>,
    meta: HistoryCursor,
}

#[derive(Debug, Serialize, Deserialize)]
struct HistoryEntry {
    date: NaiveDate,
    close: f64,
    high: f64,
    low: f64,
    volume: i64,
    facevalue: i64,
}

fn sanitize_ticker(ticker: String) -> String {
    return ticker
        .chars()
        .take(20)
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect();
}

async fn get_ticker_moex(Path(ticker): Path<String>) -> Json<serde_json::Value> {
    if let Ok(history) = MOEX_API
        .get_regular_ticker(&(sanitize_ticker(ticker)))
        .await
    {
        return Json(json!(history));
    }
    return Json(json!({"error": "something went wrong"}));
}

#[tokio::main]
async fn main() {
    // logger
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .or_else(|_| {
                    tracing_subscriber::EnvFilter::try_new("exchange_api=error,tower_http=warn")
                })
                .unwrap(),
        )
        .init();

    // app
    let app = Router::new()
        .route("/moex/{ticker}", get(get_ticker_moex))
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
