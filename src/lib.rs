use std::{io, sync::Arc};

use tokio::net::TcpListener;
use tracing::info;
use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};
use utoipa_swagger_ui::SwaggerUi;

pub use crate::{error::AppError, pdf::PdfContext};

/// OpenAPI descriptor for the Typst webservice.
#[derive(OpenApi)]
struct ApiDoc;

mod assets;
mod error;
pub mod handlers;
pub mod logging;
pub mod pdf;
mod zip;

#[cfg(test)]
mod tests;

/// Launch the HTTP server and publish the PDF rendering endpoint.
pub async fn start_typst_server(addr: String, pdf_context: PdfContext) -> Result<(), AppError> {
    let pdf_context = Arc::new(pdf_context);
    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(handlers::render_pdf, handlers::render_pdf_batch))
        .with_state(pdf_context)
        .split_for_parts();

    let router = router.merge(SwaggerUi::new("/").url("/apidoc/openapi.json", api));

    // Bind to all interfaces on the requested port
    info!("Binding HTTP listener on {}", addr);
    let listener = TcpListener::bind(&addr).await?;

    info!("HTTP listener ready; serving requests");
    if let Err(error) = axum::serve(listener, router).await {
        tracing::error!(%error, "Server encountered an error");
        return Err(io::Error::other(error).into());
    }

    Ok(())
}
