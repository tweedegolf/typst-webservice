use std::{env, io, net::Ipv4Addr, sync::Arc};

use tokio::net::TcpListener;
use tracing::info;
use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};
use utoipa_swagger_ui::SwaggerUi;

use crate::{error::AppError, pdf::PdfContext};

const DEFAULT_ASSETS_DIR: &str = "assets";
const ASSETS_DIR_ENV_VAR: &str = "TWS_DIR";
const DEFAULT_PORT: u16 = 8080;
const PORT_ENV_VAR: &str = "TWS_PORT";

/// OpenAPI descriptor for the Typst webservice.
#[derive(OpenApi)]
struct ApiDoc;

mod assets;
mod error;
pub(crate) mod handlers;
mod logging;
mod pdf;
mod zip;

#[cfg(test)]
mod tests;

#[tokio::main]
/// Launch the HTTP server and publish the PDF rendering endpoint.
async fn main() -> Result<(), AppError> {
    logging::init();
    info!("Starting Typst webservice");
    let assets_dir = resolve_assets_dir();
    info!(%assets_dir, "Loading Typst assets");
    let pdf_context = Arc::new(PdfContext::from_directory(&assets_dir)?);

    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(handlers::render_pdf, handlers::render_pdf_batch))
        .with_state(pdf_context)
        .split_for_parts();

    let router = router.merge(SwaggerUi::new("/").url("/apidoc/openapi.json", api));

    let port = env::var(PORT_ENV_VAR)
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT);

    // Bind to all interfaces on the requested port
    info!("Binding HTTP listener on 0.0.0.0:{}", port);
    let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)).await?;

    info!("HTTP listener ready; serving requests");
    if let Err(error) = axum::serve(listener, router).await {
        tracing::error!(%error, "Server encountered an error");
        return Err(io::Error::other(error).into());
    }

    Ok(())
}

/// Determine the directory containing Typst assets from CLI args or environment.
fn resolve_assets_dir() -> String {
    env::args()
        .nth(1)
        .filter(|arg| !arg.is_empty())
        .or_else(|| {
            env::var(ASSETS_DIR_ENV_VAR)
                .ok()
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_else(|| DEFAULT_ASSETS_DIR.to_string())
}
