use crate::models::IndexResponse;
use rocket::serde::json::Json;

#[rocket::get("/")]
pub fn index() -> Json<IndexResponse> {
    Json(IndexResponse {
        service: "npmv".to_string(),
        description: "NPM Registry version to simplified release array converter API".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        usage: "GET /<package-name> (e.g., /react or /@payfurl/client), GET /diff/<package-name>?version[]=v1&version[]=v2 (e.g., /diff/@payfurl/client?version[]=4.8.6&version[]=4.8.7)".to_string(),
    })
}
