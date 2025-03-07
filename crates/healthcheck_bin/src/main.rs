use reqwest;
use serde::Deserialize;

#[derive(Debug)]
enum CustomError {
    ReqwestError(String),
    NotOk,
}

#[derive(Debug, Deserialize)]
struct StatusJSON {
    status: String,
}

impl std::fmt::Display for CustomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CustomError::ReqwestError(e) => write!(f, "Reqwest error: {}", e),
            CustomError::NotOk => write!(f, "Status code != 200 or no healthcheck"),
        }
    }
}

impl From<reqwest::Error> for CustomError {
    fn from(err: reqwest::Error) -> CustomError {
        CustomError::ReqwestError(err.to_string())
    }
}

fn main() -> Result<(), CustomError> {
    let res = reqwest::blocking::get("http://localhost:8080/healthcheck")?;
    if res.status() != 200 {
        return Err(CustomError::NotOk);
    }
    let ok_str: StatusJSON = res.json::<StatusJSON>()?;
    if ok_str.status != "ok" {
        return Err(CustomError::NotOk);
    }
    Ok(())
}
