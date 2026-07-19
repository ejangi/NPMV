use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
pub struct NpmDist {
    pub tarball: String,
    pub shasum: String,
}

#[derive(Deserialize, Debug)]
pub struct NpmVersion {
    pub version: String,
    pub dist: NpmDist,
}

#[derive(Deserialize, Debug)]
pub struct NpmPackage {
    pub versions: HashMap<String, NpmVersion>,
    pub time: Option<HashMap<String, String>>,
}

#[derive(Serialize, Debug, Clone)]
pub struct Release {
    pub version: String,
    pub date_time: Option<String>,
    pub tarball: String,
    pub shasum: String,
    pub diff: Option<String>,
    pub diff_raw: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct ErrorResponse {
    pub error: String,
}

pub struct AppState {
    pub client: reqwest::Client,
}

#[derive(Serialize)]
pub struct IndexResponse {
    pub service: String,
    pub description: String,
    pub version: String,
    pub usage: String,
}
