# NPM Registry Version Array Simplifier API (npmv)

`npmv` is a lightweight Rust web application built with the Rocket framework that serves as a proxy/converter for the NPM Registry. 

It fetches package metadata directly from `https://registry.npmjs.org/<package-name>` and transforms the unordered version mapping object into a simplified, semver-sorted chronological array of releases. 

This simplifies ingestion in data integration tools and other environments where parsing a variable-keyed object mapping is inconvenient.

## API Usage

### Service Metadata / Index
```http
GET /
```
Returns service-level index and usage data.

### Query Package
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

## Build & Run using Docker

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
```

---

## Local Development (Native Rust)

Ensure you have Rust and Cargo installed, then run:
```bash
PORT=8080 cargo run
```
