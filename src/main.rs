extern crate env_logger;
use moex_api::api::MoexAPI;
use serde_json::json;
use tower_http::trace::TraceLayer;

use axum::{extract::Path, routing::get, Json, Router};

static MOEX_API: std::sync::LazyLock<MoexAPI> = std::sync::LazyLock::new(|| MoexAPI::new());

fn sanitize_ticker(ticker: String) -> String {
    return ticker
        .chars()
        .take(20)
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect();
}

async fn get_ticker_moex(Path(ticker): Path<String>) -> Json<serde_json::Value> {
    if let Ok(history) = MOEX_API.get_ticker(&(sanitize_ticker(ticker))).await {
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
