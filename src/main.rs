use rocket::http::Status;
use rocket::response::status;
use rocket::serde::json::Json;
use rocket::State;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Deserialize, Debug)]
struct NpmDist {
    tarball: String,
    shasum: String,
}

#[derive(Deserialize, Debug)]
struct NpmVersion {
    version: String,
    dist: NpmDist,
}

#[derive(Deserialize, Debug)]
struct NpmPackage {
    versions: HashMap<String, NpmVersion>,
    time: Option<HashMap<String, String>>,
}

#[derive(Serialize, Debug, Clone)]
struct Release {
    version: String,
    date_time: Option<String>,
    tarball: String,
    shasum: String,
}

#[derive(Serialize, Debug)]
struct ErrorResponse {
    error: String,
}

struct AppState {
    client: reqwest::Client,
}

#[derive(Serialize)]
struct IndexResponse {
    service: String,
    description: String,
    version: String,
    usage: String,
}

#[rocket::get("/")]
fn index() -> Json<IndexResponse> {
    Json(IndexResponse {
        service: "npmv".to_string(),
        description: "NPM Registry version to simplified release array converter API".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        usage: "GET /<package-name> (e.g., /react or /@payfurl/client)".to_string(),
    })
}

#[rocket::get("/<package..>")]
async fn get_package(
    package: PathBuf,
    state: &State<AppState>,
) -> Result<Json<Vec<Release>>, status::Custom<Json<ErrorResponse>>> {
    let package_name = package
        .iter()
        .map(|segment| segment.to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");

    if package_name.is_empty() {
        return Err(status::Custom(
            Status::BadRequest,
            Json(ErrorResponse {
                error: "Package name is empty".to_string(),
            }),
        ));
    }

    // Don't forward requests for favicon.ico to npm registry
    if package_name == "favicon.ico" {
        return Err(status::Custom(
            Status::NotFound,
            Json(ErrorResponse {
                error: "Not Found".to_string(),
            }),
        ));
    }

    let url = format!("https://registry.npmjs.org/{}", package_name);
    let response = match state.client.get(&url).send().await {
        Ok(resp) => resp,
        Err(err) => {
            return Err(status::Custom(
                Status::BadGateway,
                Json(ErrorResponse {
                    error: format!("Failed to connect to NPM registry: {}", err),
                }),
            ));
        }
    };

    if response.status() == Status::NotFound.code {
        return Err(status::Custom(
            Status::NotFound,
            Json(ErrorResponse {
                error: format!("Package '{}' not found in NPM registry", package_name),
            }),
        ));
    }

    if !response.status().is_success() {
        return Err(status::Custom(
            Status::BadGateway,
            Json(ErrorResponse {
                error: format!("NPM registry returned HTTP status: {}", response.status()),
            }),
        ));
    }

    let npm_package = match response.json::<NpmPackage>().await {
        Ok(pkg) => pkg,
        Err(err) => {
            return Err(status::Custom(
                Status::UnprocessableEntity,
                Json(ErrorResponse {
                    error: format!("Failed to parse NPM registry response: {}", err),
                }),
            ));
        }
    };

    let mut releases = Vec::new();
    for (version_str, version_info) in npm_package.versions {
        let date_time = npm_package
            .time
            .as_ref()
            .and_then(|t| t.get(&version_str).cloned());

        releases.push(Release {
            version: version_info.version,
            date_time,
            tarball: version_info.dist.tarball,
            shasum: version_info.dist.shasum,
        });
    }

    // Sort chronologically using semver
    releases.sort_by(|a, b| {
        let version_a = semver::Version::parse(&a.version);
        let version_b = semver::Version::parse(&b.version);
        match (version_a, version_b) {
            (Ok(va), Ok(vb)) => va.cmp(&vb),
            (Ok(_), Err(_)) => std::cmp::Ordering::Less,
            (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
            (Err(_), Err(_)) => a.version.cmp(&b.version),
        }
    });

    Ok(Json(releases))
}

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
        .mount("/", rocket::routes![index, get_package])
}
