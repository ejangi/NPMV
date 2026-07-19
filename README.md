# NPM Registry Version Array Simplifier & Package Diff API (npmv)

`npmv` is a lightweight Rust web application built with the Rocket framework that serves as a proxy/converter for the NPM Registry. 

It fetches package metadata directly from `https://registry.npmjs.org/<package-name>` to deliver semver-sorted release lists and on-the-fly tarball package diffs.

## API Usage

### Service Metadata / Index
```http
GET /
```
Returns service-level index and usage data.

### Query Package Versions
```http
GET /<package-name>
```
Works for both unscoped packages (e.g., `/react`) and scoped packages (e.g., `/@payfurl/client`).

#### Example Output (`GET /react`):
```json
[
  {
    "version": "19.0.0",
    "date_time": "2024-12-05T17:15:33.123Z",
    "tarball": "https://registry.npmjs.org/react/-/react-19.0.0.tgz",
    "shasum": "87bf6d5..."
  }
]
```

---

### Package Version Diff
```http
GET /diff/<package-name>?version[]=v1&version[]=v2
```
Downloads the tarballs for the specified versions from the NPM registry, extracts their contents in-memory, and generates a unified text diff showing all file additions, deletions, and code modifications between the two versions.

#### Flexible Query Parameter Styles:
- **Array syntax**: `/diff/@payfurl/client?version[]=4.8.6&version[]=4.8.7`
- **Repeated parameter**: `/diff/@payfurl/client?version=4.8.6&version=4.8.7`
- **Comma-separated**: `/diff/@payfurl/client?version=4.8.6,4.8.7`
- **Explicit bounds**: `/diff/@payfurl/client?from=4.8.6&to=4.8.7` or `?v1=4.8.6&v2=4.8.7`

#### Options:
- **`raw`** *(default: `false`)*: By default, `/diff` renders a rich, GitLab-style HTML diff interface (with green/red line highlighting, side-by-side or line-by-line view toggles, and expandable file sections). Pass `raw=true` to return the plain-text unified diff.
- **`include_node_modules`** *(default: `false`)*: By default, files and code regions inside `node_modules` are excluded from the diff. Pass `include_node_modules=true` to include them.

#### Example Usage:
```bash
curl -s "http://localhost:8080/diff/@payfurl/client?version[]=4.8.6&version[]=4.8.7"
```

#### Example Output:
```diff
--- a/package.json
+++ b/package.json
@@ -3,4 +3,4 @@
   "name": "@payfurl/client",
-  "version": "4.8.6",
+  "version": "4.8.7",
```

---

## Fast Local Development using Docker Compose

Docker Compose is configured specifically for fast local development:
- Source code is live-mounted directly into the container (`.:/app`).
- Cargo registry and build target folders use persistent named volumes (`cargo_registry` and `cargo_target`), preserving build artifacts across container restarts.

### 1. Start the Service
```bash
docker compose up
```
*(The initial run downloads dependencies and builds the debug target. Subsequent restarts compile only changed files incrementally).*

### 2. Refresh / Recompile Code Changes
When you edit `src/main.rs`, simply restart the app container (takes ~1-2 seconds):
```bash
docker compose restart app
```

---

## Build & Run using Docker (Manual)

The application includes a multi-stage Dockerfile that compiles the Rust code in a cached builder stage and serves the binary from a minimal, secure `debian:bookworm-slim` image.

### 1. Build the Docker Image
Run this command from the root directory of the project:
```bash
docker build -t npmv .
```

### 2. Run the Docker Container
Start the container on port `8080`:
```bash
docker run -d -p 8080:8080 --name npmv-api npmv
```

### 3. Test the Running API
Run a test query:
```bash
curl -s http://localhost:8080/react
curl -s "http://localhost:8080/diff/react?version[]=19.0.0&version[]=19.0.1"
```

---

## Local Development (Native Rust)

Ensure you have Rust and Cargo installed, then run:
```bash
PORT=8080 cargo run
```
