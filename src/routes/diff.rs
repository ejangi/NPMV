use crate::models::{AppState, ErrorResponse, NpmPackage};
use rocket::form::FromForm;
use rocket::http::{ContentType, Status};
use rocket::response::{status, Responder};
use rocket::serde::json::Json;
use rocket::State;
use similar::TextDiff;
use std::collections::{BTreeSet, HashMap};
use std::io::Read;
use std::path::PathBuf;

#[derive(FromForm, Debug)]
pub struct DiffQuery {
    pub version: Option<Vec<String>>,
    pub v: Option<Vec<String>>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub v1: Option<String>,
    pub v2: Option<String>,
    pub include_node_modules: Option<bool>,
    pub raw: Option<bool>,
}

impl DiffQuery {
    pub fn extract_versions(&self) -> Vec<String> {
        let mut raw_versions = Vec::new();

        if let Some(ref list) = self.version {
            raw_versions.extend(list.clone());
        }
        if let Some(ref list) = self.v {
            raw_versions.extend(list.clone());
        }
        if let Some(ref f) = self.from {
            raw_versions.push(f.clone());
        }
        if let Some(ref t) = self.to {
            raw_versions.push(t.clone());
        }
        if let Some(ref v1) = self.v1 {
            raw_versions.push(v1.clone());
        }
        if let Some(ref v2) = self.v2 {
            raw_versions.push(v2.clone());
        }

        let mut final_versions = Vec::new();
        for item in raw_versions {
            for part in item.split(',') {
                let trimmed = part.trim();
                if !trimmed.is_empty() {
                    final_versions.push(trimmed.to_string());
                }
            }
        }
        final_versions
    }
}

pub enum DiffResponse {
    Text(String),
    Html(String),
}

impl<'r> Responder<'r, 'static> for DiffResponse {
    fn respond_to(self, request: &'r rocket::Request<'_>) -> rocket::response::Result<'static> {
        match self {
            DiffResponse::Text(text) => {
                (Status::Ok, (ContentType::Plain, text)).respond_to(request)
            }
            DiffResponse::Html(html) => {
                (Status::Ok, (ContentType::HTML, html)).respond_to(request)
            }
        }
    }
}

pub fn is_binary(bytes: &[u8]) -> bool {
    let sample_size = bytes.len().min(8192);
    bytes[..sample_size].contains(&0) || std::str::from_utf8(bytes).is_err()
}

pub fn strip_node_modules_regions(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut in_node_modules_region = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("//#region node_modules/")
            || trimmed.starts_with("//#region \\0")
            || trimmed.starts_with("// node_modules/")
            || trimmed.starts_with("/* node_modules/")
        {
            in_node_modules_region = true;
            continue;
        }

        if in_node_modules_region {
            if trimmed == "//#endregion"
                || trimmed == "/* #endregion */"
                || trimmed.ends_with("#endregion")
            {
                in_node_modules_region = false;
            }
            continue;
        }

        if trimmed.contains("node_modules/")
            && (trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("*"))
        {
            continue;
        }

        result.push_str(line);
        result.push('\n');
    }

    result
}

pub fn extract_tarball(
    bytes: &[u8],
    include_node_modules: bool,
) -> Result<HashMap<PathBuf, Vec<u8>>, String> {
    let gz = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(gz);
    let mut files = HashMap::new();

    let entries = archive
        .entries()
        .map_err(|e| format!("Failed to read archive entries: {}", e))?;

    for entry_res in entries {
        let mut entry = match entry_res {
            Ok(e) => e,
            Err(e) => return Err(format!("Corrupt archive entry: {}", e)),
        };

        let path = match entry.path() {
            Ok(p) => p.to_path_buf(),
            Err(_) => continue,
        };

        if entry.header().entry_type().is_dir() {
            continue;
        }

        let rel_path = match path.strip_prefix("package") {
            Ok(p) => p.to_path_buf(),
            Err(_) => path,
        };

        if !include_node_modules
            && rel_path
                .components()
                .any(|c| c.as_os_str() == "node_modules")
        {
            continue;
        }

        let mut contents = Vec::new();
        if let Err(e) = entry.read_to_end(&mut contents) {
            return Err(format!("Failed to read file in archive: {}", e));
        }

        files.insert(rel_path, contents);
    }

    Ok(files)
}

pub fn diff_files(
    path_str: &str,
    old_content: Option<&[u8]>,
    new_content: Option<&[u8]>,
    include_node_modules: bool,
) -> Option<String> {
    match (old_content, new_content) {
        (Some(old_bytes), Some(new_bytes)) => {
            if old_bytes == new_bytes {
                return None;
            }
            if is_binary(old_bytes) || is_binary(new_bytes) {
                return Some(format!(
                    "diff -r a/{} b/{}\nBinary files a/{} and b/{} differ\n",
                    path_str, path_str, path_str, path_str
                ));
            }
            let old_raw = String::from_utf8_lossy(old_bytes);
            let new_raw = String::from_utf8_lossy(new_bytes);

            let (old_str, new_str) = if include_node_modules {
                (old_raw, new_raw)
            } else {
                (
                    std::borrow::Cow::Owned(strip_node_modules_regions(&old_raw)),
                    std::borrow::Cow::Owned(strip_node_modules_regions(&new_raw)),
                )
            };

            if old_str == new_str {
                return None;
            }

            let diff = TextDiff::from_lines(old_str.as_ref(), new_str.as_ref());
            let unified = diff
                .unified_diff()
                .context_radius(3)
                .header(&format!("a/{}", path_str), &format!("b/{}", path_str))
                .to_string();

            if unified.trim().is_empty() {
                None
            } else {
                Some(unified)
            }
        }
        (Some(old_bytes), None) => {
            if is_binary(old_bytes) {
                return Some(format!(
                    "diff -r a/{} /dev/null\nBinary file a/{} deleted\n",
                    path_str, path_str
                ));
            }
            let old_raw = String::from_utf8_lossy(old_bytes);
            let old_str = if include_node_modules {
                old_raw
            } else {
                std::borrow::Cow::Owned(strip_node_modules_regions(&old_raw))
            };

            if old_str.trim().is_empty() {
                return None;
            }

            let diff = TextDiff::from_lines(old_str.as_ref(), "");
            let unified = diff
                .unified_diff()
                .context_radius(3)
                .header(&format!("a/{}", path_str), "/dev/null")
                .to_string();
            Some(unified)
        }
        (None, Some(new_bytes)) => {
            if is_binary(new_bytes) {
                return Some(format!(
                    "diff -r /dev/null b/{}\nBinary file b/{} created\n",
                    path_str, path_str
                ));
            }
            let new_raw = String::from_utf8_lossy(new_bytes);
            let new_str = if include_node_modules {
                new_raw
            } else {
                std::borrow::Cow::Owned(strip_node_modules_regions(&new_raw))
            };

            if new_str.trim().is_empty() {
                return None;
            }

            let diff = TextDiff::from_lines("", new_str.as_ref());
            let unified = diff
                .unified_diff()
                .context_radius(3)
                .header("/dev/null", &format!("b/{}", path_str))
                .to_string();
            Some(unified)
        }
        (None, None) => None,
    }
}

fn render_html_page(
    package_name: &str,
    v1: &str,
    v2: &str,
    diff_json: &str,
    raw_url: &str,
) -> String {
    let mut html = String::with_capacity(diff_json.len() + 4000);
    html.push_str(r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Diff: "##);
    html.push_str(package_name);
    html.push_str(" (");
    html.push_str(v1);
    html.push_str(" &rarr; ");
    html.push_str(v2);
    html.push_str(r##")</title>
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/diff2html/bundles/css/diff2html.min.css" />
  <link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500&display=swap" />
  <style>
    :root {
      --bg-color: #f8f9fa;
      --card-bg: #ffffff;
      --text-color: #2e2e38;
      --border-color: #e5e5ed;
      --primary: #e24329;
      --primary-hover: #c0341d;
      --addition-bg: #ecf7ed;
      --addition-line: #cdedd0;
      --addition-text: #116620;
      --deletion-bg: #fcf0f0;
      --deletion-line: #f7d4d4;
      --deletion-text: #af1919;
    }

    body {
      margin: 0;
      padding: 0;
      font-family: 'Inter', -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      background-color: var(--bg-color);
      color: var(--text-color);
    }

    .header-bar {
      position: sticky;
      top: 0;
      z-index: 100;
      background: #ffffff;
      border-bottom: 1px solid var(--border-color);
      padding: 12px 24px;
      display: flex;
      align-items: center;
      justify-content: space-between;
      box-shadow: 0 1px 3px rgba(0,0,0,0.05);
    }

    .header-title {
      display: flex;
      align-items: center;
      gap: 12px;
      font-size: 1.1rem;
      font-weight: 600;
    }

    .package-name {
      color: #1f1e24;
      font-family: 'JetBrains Mono', monospace;
    }

    .version-badge {
      background: #e9ecef;
      color: #495057;
      padding: 4px 8px;
      border-radius: 6px;
      font-size: 0.85rem;
      font-family: 'JetBrains Mono', monospace;
      font-weight: 500;
    }

    .controls {
      display: flex;
      align-items: center;
      gap: 12px;
    }

    .btn {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      padding: 6px 14px;
      font-size: 0.875rem;
      font-weight: 500;
      color: #333238;
      background-color: #ffffff;
      border: 1px solid #d1d1d8;
      border-radius: 6px;
      text-decoration: none;
      cursor: pointer;
      transition: all 0.15s ease;
    }

    .btn:hover {
      background-color: #f2f2f5;
      border-color: #b0b0b8;
    }

    .main-container {
      max-width: 1400px;
      margin: 24px auto;
      padding: 0 24px;
    }

    .d2h-file-wrapper {
      border: 1px solid var(--border-color) !important;
      border-radius: 8px !important;
      margin-bottom: 20px !important;
      box-shadow: 0 1px 2px rgba(0,0,0,0.03) !important;
      overflow: hidden;
    }

    .d2h-file-header {
      background-color: #fafafa !important;
      border-bottom: 1px solid var(--border-color) !important;
      padding: 10px 16px !important;
      font-family: 'JetBrains Mono', monospace !important;
      font-size: 0.9rem !important;
    }

    .d2h-file-name {
      color: #2e2e38 !important;
      font-weight: 600 !important;
    }

    .d2h-del {
      background-color: var(--deletion-bg) !important;
      border-color: var(--deletion-line) !important;
    }
    .d2h-del td {
      color: var(--deletion-text) !important;
    }

    .d2h-ins {
      background-color: var(--addition-bg) !important;
      border-color: var(--addition-line) !important;
    }
    .d2h-ins td {
      color: var(--addition-text) !important;
    }

    .d2h-code-line del {
      background-color: #f7b0b0 !important;
      text-decoration: none !important;
      border-radius: 2px;
    }

    .d2h-code-line ins {
      background-color: #a3e9b3 !important;
      text-decoration: none !important;
      border-radius: 2px;
    }

    .d2h-code-line, .d2h-code-linenumber {
      font-family: 'JetBrains Mono', monospace !important;
      font-size: 0.85rem !important;
    }

    .d2h-code-linenumber {
      color: #898894 !important;
      background-color: #fafafa !important;
      border-right: 1px solid var(--border-color) !important;
    }
  </style>
</head>
<body>
  <div class="header-bar">
    <div class="header-title">
      <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="18" cy="18" r="3"></circle>
        <circle cx="6" cy="6" r="3"></circle>
        <path d="M13 6h3a2 2 0 0 1 2 2v7"></path>
        <line x1="6" y1="9" x2="6" y2="21"></line>
      </svg>
      <span class="package-name">"##);
    html.push_str(package_name);
    html.push_str(r##"</span>
      <span class="version-badge">"##);
    html.push_str(v1);
    html.push_str(" &rarr; ");
    html.push_str(v2);
    html.push_str(r##"</span>
    </div>
    <div class="controls">
      <button id="toggle-view" class="btn">Side-by-Side View</button>
      <a href=""##);
    html.push_str(raw_url);
    html.push_str(r##"" class="btn">Raw Diff</a>
    </div>
  </div>

  <div class="main-container">
    <div id="diff-target"></div>
  </div>

  <script src="https://cdn.jsdelivr.net/npm/diff2html/bundles/js/diff2html-ui.min.js"></script>
  <script>
    const diffString = "##);
    html.push_str(diff_json);
    html.push_str(r##";
    const targetElement = document.getElementById("diff-target");
    let currentOutputType = "line-by-line";

    function renderDiff(outputType) {
      const diff2htmlUi = new Diff2HtmlUI(targetElement, diffString, {
        drawFileList: true,
        matching: "lines",
        outputFormat: outputType,
        renderNothingWhenEmpty: false,
      });
      diff2htmlUi.draw();
    }

    renderDiff(currentOutputType);

    document.getElementById("toggle-view").addEventListener("click", function() {
      if (currentOutputType === "line-by-line") {
        currentOutputType = "side-by-side";
        this.textContent = "Line-by-Line View";
      } else {
        currentOutputType = "line-by-line";
        this.textContent = "Side-by-Side View";
      }
      renderDiff(currentOutputType);
    });
  </script>
</body>
</html>"##);
    html
}

#[rocket::get("/diff/<package..>?<query..>")]
pub async fn get_diff(
    package: PathBuf,
    query: Option<DiffQuery>,
    state: &State<AppState>,
) -> Result<DiffResponse, status::Custom<Json<ErrorResponse>>> {
    let package_name = package
        .iter()
        .map(|segment| segment.to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");

    if package_name.is_empty() || package_name == "favicon.ico" {
        return Err(status::Custom(
            Status::BadRequest,
            Json(ErrorResponse {
                error: "Package name is required".to_string(),
            }),
        ));
    }

    let versions = query
        .as_ref()
        .map(|q| q.extract_versions())
        .unwrap_or_default();

    if versions.len() != 2 {
        return Err(status::Custom(
            Status::BadRequest,
            Json(ErrorResponse {
                error: "Exactly two version parameters are required (e.g. ?version[]=4.8.6&version[]=4.8.7 or ?version=4.8.6&version=4.8.7)".to_string(),
            }),
        ));
    }

    let include_node_modules = query
        .as_ref()
        .and_then(|q| q.include_node_modules)
        .unwrap_or(false);

    let raw = query.as_ref().and_then(|q| q.raw).unwrap_or(false);

    let v1 = &versions[0];
    let v2 = &versions[1];

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

    let v1_info = match npm_package.versions.get(v1) {
        Some(info) => info,
        None => {
            return Err(status::Custom(
                Status::NotFound,
                Json(ErrorResponse {
                    error: format!("Version '{}' not found for package '{}'", v1, package_name),
                }),
            ));
        }
    };

    let v2_info = match npm_package.versions.get(v2) {
        Some(info) => info,
        None => {
            return Err(status::Custom(
                Status::NotFound,
                Json(ErrorResponse {
                    error: format!("Version '{}' not found for package '{}'", v2, package_name),
                }),
            ));
        }
    };

    let (bytes1_res, bytes2_res) = tokio::join!(
        state.client.get(&v1_info.dist.tarball).send(),
        state.client.get(&v2_info.dist.tarball).send(),
    );

    let resp1 = match bytes1_res {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            return Err(status::Custom(
                Status::BadGateway,
                Json(ErrorResponse {
                    error: format!("Failed to download tarball for version {}: HTTP {}", v1, r.status()),
                }),
            ));
        }
        Err(e) => {
            return Err(status::Custom(
                Status::BadGateway,
                Json(ErrorResponse {
                    error: format!("Failed to download tarball for version {}: {}", v1, e),
                }),
            ));
        }
    };

    let resp2 = match bytes2_res {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            return Err(status::Custom(
                Status::BadGateway,
                Json(ErrorResponse {
                    error: format!("Failed to download tarball for version {}: HTTP {}", v2, r.status()),
                }),
            ));
        }
        Err(e) => {
            return Err(status::Custom(
                Status::BadGateway,
                Json(ErrorResponse {
                    error: format!("Failed to download tarball for version {}: {}", v2, e),
                }),
            ));
        }
    };

    let (b1_res, b2_res) = tokio::join!(resp1.bytes(), resp2.bytes());
    let bytes1 = b1_res.map_err(|e| {
        status::Custom(
            Status::BadGateway,
            Json(ErrorResponse {
                error: format!("Failed to read tarball body for version {}: {}", v1, e),
            }),
        )
    })?;
    let bytes2 = b2_res.map_err(|e| {
        status::Custom(
            Status::BadGateway,
            Json(ErrorResponse {
                error: format!("Failed to read tarball body for version {}: {}", v2, e),
            }),
        )
    })?;

    let files1 = match extract_tarball(&bytes1, include_node_modules) {
        Ok(f) => f,
        Err(e) => {
            return Err(status::Custom(
                Status::UnprocessableEntity,
                Json(ErrorResponse {
                    error: format!("Failed to extract tarball for version {}: {}", v1, e),
                }),
            ));
        }
    };

    let files2 = match extract_tarball(&bytes2, include_node_modules) {
        Ok(f) => f,
        Err(e) => {
            return Err(status::Custom(
                Status::UnprocessableEntity,
                Json(ErrorResponse {
                    error: format!("Failed to extract tarball for version {}: {}", v2, e),
                }),
            ));
        }
    };

    let mut all_paths: BTreeSet<PathBuf> = BTreeSet::new();
    for p in files1.keys() {
        all_paths.insert(p.clone());
    }
    for p in files2.keys() {
        all_paths.insert(p.clone());
    }

    let mut diff_output = String::new();

    for path in all_paths {
        let path_str = path.to_string_lossy().replace('\\', "/");
        let f1 = files1.get(&path).map(|v| v.as_slice());
        let f2 = files2.get(&path).map(|v| v.as_slice());

        if let Some(diff_chunk) = diff_files(&path_str, f1, f2, include_node_modules) {
            diff_output.push_str(&diff_chunk);
        }
    }

    if diff_output.is_empty() {
        diff_output = format!(
            "# Package: {}\n# Version {} -> {}\n# No differences found.\n",
            package_name, v1, v2
        );
    }

    if raw {
        Ok(DiffResponse::Text(diff_output))
    } else {
        let diff_json = serde_json::to_string(&diff_output).unwrap_or_else(|_| "\"\"".to_string());
        let raw_url = format!(
            "/diff/{}?version[]={}&version[]={}{}&raw=true",
            package_name,
            v1,
            v2,
            if include_node_modules {
                "&include_node_modules=true"
            } else {
                ""
            }
        );
        let html = render_html_page(&package_name, v1, v2, &diff_json, &raw_url);
        Ok(DiffResponse::Html(html))
    }
}
