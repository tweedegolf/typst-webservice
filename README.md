# Typst Webservice

Typst Webservice exposes a small Axum-based HTTP API that compiles Typst templates into PDFs given a JSON input. Templates, assets, and fonts are preloaded from an on-disk directory.

## Features

- `GET /render-pdf/{template}/{file_name}` renders a single template into PDF.
- `POST /render-pdf/batch` renders multiple templates and returns a streaming ZIP archive.
- Streaming ZIP writer keeps memory usage predictable for large batches.
- Detailed error responses include unique reference IDs for troubleshooting.
- Structured logging powered by `tracing`.

## Getting Started

```bash
cargo run
```

By default the server loads templates from the `assets/` directory in the project root and binds to `127.0.0.1:8080`.

### Choosing a asset directory

You can point the service at a different assets directory using either a command-line argument or an environment variable:

```bash
# Command-line override
cargo run -- ./my-templates

# Environment variable override
TWS_DIR=./my-templates cargo run
```

The command-line argument takes precedence; both fall back to `assets/` when unset.

## API Documentation

The service ships with an OpenAPI description and Swagger UI. Once the server is running, open http://127.0.0.1:8080/ in your browser to explore and try out the endpoints.

## Running Tests

```bash
cargo test
```

Integration tests exercise both single and batch rendering flows using fixtures from the `assets/` directory.
