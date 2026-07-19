use crate::models::{AppState, ErrorResponse, NpmPackage, Release};
use rocket::http::Status;
use rocket::response::status;
use rocket::serde::json::Json;
use rocket::State;
use std::path::PathBuf;

#[rocket::get("/<package..>")]
pub async fn get_package(
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
            diff: None,
            diff_raw: None,
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

    // Generate diff and diff_raw URIs relative to the previous version
    for i in 1..releases.len() {
        let prev_version = releases[i - 1].version.clone();
        let curr_version = releases[i].version.clone();
        releases[i].diff = Some(format!(
            "/diff/{}?version[]={}&version[]={}",
            package_name, prev_version, curr_version
        ));
        releases[i].diff_raw = Some(format!(
            "/diff/{}?version[]={}&version[]={}&raw=true",
            package_name, prev_version, curr_version
        ));
    }

    Ok(Json(releases))
}
