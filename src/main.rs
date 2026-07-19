mod models;
mod routes;

#[cfg(test)]
mod tests;

use models::AppState;

#[rocket::launch]
fn rocket() -> _ {
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let config = rocket::Config {
        port,
        address: std::net::Ipv4Addr::UNSPECIFIED.into(), // Bind to 0.0.0.0 for Cloud Run
        ..rocket::Config::default()
    };

    let client = reqwest::Client::builder()
        .user_agent("npmv/0.1.0 (NPM Registry Version Array Simplifier API)")
        .build()
        .expect("Failed to build reqwest client");

    rocket::custom(config)
        .manage(AppState { client })
        .mount(
            "/",
            rocket::routes![
                routes::index::index,
                routes::package::get_package,
                routes::diff::get_diff,
            ],
        )
}
