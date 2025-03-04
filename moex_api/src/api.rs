use chrono::NaiveDate;
use history_model::HistoryEntry;
use log::debug;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;

const DEFAULT_PAGE_SIZE: i64 = 100;
const MOEX_BASE_API_URL: &str = "https://iss.moex.com";

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

#[derive(Clone)]
pub struct MoexAPI {
    base_url: &'static str,
    client: reqwest::Client,
    redis_client: redis::Client,
}

impl MoexAPI {
    pub fn new(redis_client: redis::Client) -> Self {
        return MoexAPI {
            base_url: MOEX_BASE_API_URL,
            client: reqwest::Client::new(),
            redis_client,
        };
    }

    pub async fn get_ticker(&self, ticker: &str) -> Result<Vec<HistoryEntry>, Box<dyn Error>> {
        let mut redis_con = self.redis_client.get_multiplexed_async_connection().await?;

        let params = self
            .get_security_parameters(&ticker, &mut redis_con)
            .await?;
        let mut total = DEFAULT_PAGE_SIZE;
        let mut offset: i64 = 0;
        let mut history = Vec::new();

        while offset < total {
            let entry_history = self
                .get_security_history_offset(&ticker, &params, offset, &mut redis_con)
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

    async fn get_security_parameters(
        &self,
        ticker: &str,
        redis_con: &mut redis::aio::MultiplexedConnection,
    ) -> Result<MoexSecurityParameters, Box<dyn Error>> {
        let url = format!(
            "{}/iss/securities/{}.json?iss.only=boards&iss.meta=off&boards.columns=boardid,market,engine,is_primary",
            self.base_url, ticker
        );

        debug!("get_security_parameters | url: {}", url);

        if redis_con.exists(&url).await? {
            debug!("get_security_parameters | cache hit | key: {}", url);
            let cached_params_str: String = redis_con.get(&url).await?;
            let cached_param: MoexSecurityParameters = serde_json::from_str(&cached_params_str)?;
            return Ok(cached_param);
        }

        debug!("get_security_parameters | cache miss | url: {}", url);

        let moex_json = self
            .client
            .get(&url)
            .send()
            .await?
            .json::<MoexSecurityParametersJSON>()
            .await?;

        for entry in moex_json.boards.data {
            if entry.3 == 1 {
                let params = MoexSecurityParameters {
                    board: entry.0,
                    market: entry.1,
                    engine: entry.2,
                };

                debug!("get_security_parameters | saving to cache");
                let serialized = serde_json::to_string(&params)?;
                let _: () = redis_con.set(&url, &serialized).await?;

                return Ok(params);
            }
        }

        Err(Box::new(CustomError::NotFound))
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

        debug!("get_security_current_price | url: {}", url);

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
        redis_con: &mut redis::aio::MultiplexedConnection,
    ) -> Result<HistoryEntriesMoexMeta, Box<dyn Error>> {
        let url = format!(
                    "{}/iss/history/engines/{}/markets/{}/boards/{}/securities/{}.json?iss.meta=off&start={}&history.columns=TRADEDATE,CLOSE,HIGH,LOW,VOLUME,FACEVALUE",
                    self.base_url, params.engine, params.market, params.board, ticker, offset
                );

        debug!("get_security_history_offset | url: {}", url);

        if redis_con.exists(&url).await? {
            debug!("get_security_history_offset | cache hit | key: {}", url);
            let cached: String = redis_con.get(&url).await?;
            let cached_data: HistoryEntriesMoexMeta = serde_json::from_str(&cached)?;
            return Ok(cached_data);
        }

        debug!("get_security_history_offset | cache miss | url: {}", url);

        let json = self
            .client
            .get(&url)
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
            let date =
                NaiveDate::parse_from_str(entry[0].as_str().unwrap_or_default(), "%Y-%m-%d")?;
            // handle obligations
            let facevalue = match entry.len() {
                // if FACEVALUE exists then len is 6, so we need to handle it
                6 => entry[5].as_i64().unwrap_or(1),
                // otherwise set 1
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

        let out = HistoryEntriesMoexMeta { history, meta };

        if out.history.len() != 0 && out.history.len() as i64 % out.meta.page_size == 0 {
            debug!("get_security_parameters | saving to cache");
            let serialized = serde_json::to_string(&out)?;
            let _: () = redis_con.set(&url, &serialized).await?;
        }

        Ok(out)
    }
}

#[derive(Debug)]
pub enum CustomError {
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
