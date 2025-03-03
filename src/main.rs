use log::{error, info};
use moex_api::api::MoexAPI;
use redis::ConnectionLike;
use serde::Serialize;
use std::{env, process::exit};

use actix_web::{get, middleware::Logger, web, App, HttpServer, Responder};

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

#[get("/healthcheck")]
async fn healthcheck() -> impl Responder {
    web::Json(HealthcheckResponse {
        status: "ok".to_string(),
    })
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // create redis connection
    let redis_url = env::var("REDIS_URL").expect("REDIS_URL must be set in the environment");
    let mut redis_client = redis::Client::open(redis_url).expect("Failed to create Redis client");
    let redis_connected = redis_client.check_connection();
    if !redis_connected {
        error!("Redis unavailable");
        exit(1);
    }
    info!("Redis connected");

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(MoexAPI::new(redis_client.clone())))
            .service(healthcheck)
            .service(get_ticker_moex)
            .wrap(Logger::default())
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
