use cbr_api::api::CbrAPI;
use log::{error, info};
use moex_api::api::MoexAPI;
use redis::ConnectionLike;
use serde::Serialize;
use spbex_api::api::SpbexAPI;
use std::{env, process::exit};

use actix_web::{get, middleware::Logger, web, App, HttpServer, Responder};

use history_model::HistoryEntry;

mod utils;

#[derive(Serialize)]
struct HealthcheckResponse {
    status: String,
}

#[get("/moex/{ticker}")]
async fn get_ticker_moex(ticker: web::Path<String>, api: web::Data<MoexAPI>) -> impl Responder {
    let sanitized_ticker = utils::sanitize_ticker(ticker.to_string());
    if let Ok(history) = api.get_ticker(&sanitized_ticker).await {
        return web::Json(history);
    }
    web::Json(vec![])
}

#[get("/spbex/{ticker}")]
async fn get_ticker_spbex(ticker: web::Path<String>, api: web::Data<SpbexAPI>) -> impl Responder {
    let sanitized_ticker = utils::sanitize_ticker(ticker.to_string());
    if let Ok(history) = api.get_ticker(&sanitized_ticker).await {
        return web::Json(history);
    }
    web::Json(vec![] as Vec<HistoryEntry>)
}

#[get("/cbr/{ticker}")]
async fn get_ticker_cbr(ticker: web::Path<String>, api: web::Data<CbrAPI>) -> impl Responder {
    let sanitized_ticker = utils::sanitize_ticker(ticker.to_string());
    if let Ok(history) = api.get_ticker(&sanitized_ticker).await {
        return web::Json(history);
    }
    web::Json(vec![])
}

#[get("/healthcheck")]
async fn healthcheck() -> impl Responder {
    web::Json(HealthcheckResponse {
        status: "ok".to_string(),
    })
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let workers_str = env::var("EXCHANGE_API_WORKERS").unwrap_or("2".to_string());
    let workers: usize = workers_str.parse().unwrap_or(2);

    // create redis connection
    let redis_url = env::var("EXCHANGE_API_REDIS")
        .expect("EXCHANGE_API_REDIS must be set with valid REDIS url");
    let mut redis_client = redis::Client::open(redis_url).expect("Failed to create Redis client");
    let redis_connected = redis_client.check_connection();
    if !redis_connected {
        error!("Redis unavailable");
        exit(1);
    }
    info!("Redis connected");

    let moex_api = web::Data::new(MoexAPI::new(redis_client));
    let spbex_api = web::Data::new(SpbexAPI::new());
    let cbr_api = web::Data::new(CbrAPI::new());

    HttpServer::new(move || {
        App::new()
            .app_data(moex_api.clone())
            .app_data(spbex_api.clone())
            .app_data(cbr_api.clone())
            .service(healthcheck)
            .service(get_ticker_moex)
            .service(get_ticker_spbex)
            .service(get_ticker_cbr)
            .wrap(Logger::default())
    })
    .bind(("0.0.0.0", 8080))?
    .workers(workers)
    .run()
    .await
}
