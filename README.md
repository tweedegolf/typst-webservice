# Typst Webservice

Typst Webservice compiles Typst templates into PDFs given a JSON input. Templates, assets, and fonts are preloaded from an on-disk directory. It ships as both a library (`PdfContext::render`, `PdfContext::render_batch`) and an optional Axum-based HTTP server.

## Features

- `GET /render-pdf/{template}/{file_name}` renders a single template into PDF.
- `POST /render-pdf/batch` renders multiple templates and returns a streaming ZIP archive.
- Streaming ZIP writer keeps memory usage predictable for large batches.
- Detailed error responses include unique reference IDs for troubleshooting.
- Structured logging powered by `tracing`.

## Cargo features

- `server` (default): enables the Axum-based HTTP server, the `typst-webservice` binary, and the `handlers` module. Disable with `default-features = false` to use the library without any HTTP dependencies:

  ```toml
  [dependencies]
  typst-webservice = { version = "0.5", default-features = false }
  ```

  The library-only build still exposes `PdfContext::render` (single PDF) and `PdfContext::render_batch` (streaming ZIP of PDFs) alongside the lower-level `render_batch_to_writer` for writing into a user-provided `AsyncWrite`.

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

## Using as a library

With `default-features = false` the crate has no HTTP dependencies and exposes just the rendering pipeline.

### Loading a `PdfContext`

A `PdfContext` holds all Typst sources, fonts, and binary assets in memory. Build one from a directory or from in-memory tuples:

```rust
use std::sync::Arc;
use typst_webservice::PdfContext;

// From a directory on disk.
let context = PdfContext::from_directory("./assets")?;

// Or from in-memory files (e.g. embedded via `include_bytes!`).
let context = PdfContext::from_assets(&[
    ("example.typ", include_bytes!("../assets/example.typ")),
    ("Bagnard.otf", include_bytes!("../assets/Bagnard.otf")),
])?;

// Share the context between render calls.
let context = Arc::new(context);
```

### Rendering a single PDF

`PdfContext::render` takes the template file name, a `serde_json::Value` payload (exposed inside the template as `input.json`), and returns the PDF bytes:

```rust
use std::sync::Arc;
use typst_webservice::PdfContext;

let context = Arc::new(PdfContext::from_directory("./assets")?);

let pdf_bytes = PdfContext::render(
    context,
    "example.typ".to_string(),
    serde_json::json!({
        "name": "World",
        "list": ["Memory Safety", "Open Source", "World Peace"],
    }),
)?;

std::fs::write("out.pdf", pdf_bytes)?;
```

`render` runs a synchronous Typst compile; call it from a blocking context (or wrap it in `tokio::task::spawn_blocking` when running inside an async runtime).

### Rendering a batch as a ZIP archive

`PdfContext::render_batch` renders many templates in parallel and returns a byte stream of the ZIP archive. Bytes are emitted as soon as each PDF is written into the archive, so the whole archive never sits in memory and callers can pipe the stream straight to a client:

```rust
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio_util::io::StreamReader;
use typst_webservice::{BatchRenderRequest, PdfContext};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = Arc::new(PdfContext::from_directory("./assets")?);

    let requests = vec![
        BatchRenderRequest {
            template: "example.typ".to_string(),
            file_name: "first.pdf".to_string(),
            input: serde_json::json!({ "name": "One", "list": ["Item"] }),
        },
        BatchRenderRequest {
            template: "example.typ".to_string(),
            file_name: "second.pdf".to_string(),
            input: serde_json::json!({ "name": "Two", "list": ["Item"] }),
        },
    ];

    let stream = PdfContext::render_batch(context, requests)?;

    // Forward the stream wherever you like — e.g. into an async writer:
    let mut reader = StreamReader::new(stream);
    let mut file = tokio::fs::File::create("out.zip").await?;
    tokio::io::copy(&mut reader, &mut file).await?;
    Ok(())
}
```

If any request references a template that is not loaded in the context, `render_batch` returns `AppError::MainSourceNotFound` synchronously, before any bytes are produced — letting HTTP callers respond with a 4xx instead of a half-written body. The call requires a Tokio runtime because rendering happens on a `spawn_blocking` pool and the ZIP is written through an async writer in a background task.

### Writing into your own `AsyncWrite`

If you'd rather write the archive directly into a sink you already own — a file, an upload, a custom transport — use the lower-level `render_batch_to_writer` with any `tokio::io::AsyncWrite`:

```rust
use std::sync::Arc;
use tokio::fs::File;
use typst_webservice::{BatchRenderRequest, PdfContext};
use typst_webservice::zip::ZipResponseWriter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = Arc::new(PdfContext::from_directory("./assets")?);
    let requests: Vec<BatchRenderRequest> = /* ... */ vec![];

    let file = File::create("out.zip").await?;
    let writer = ZipResponseWriter::new(file);
    PdfContext::render_batch_to_writer(context, requests, writer).await?;
    Ok(())
}
```

`render_batch_to_writer` finishes (and shuts down) the writer before returning it, so the archive is complete as soon as the call resolves.

## Running Tests

```bash
cargo test
```

Integration tests exercise both single and batch rendering flows using fixtures from the `assets/` directory.
