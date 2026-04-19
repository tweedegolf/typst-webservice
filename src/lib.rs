pub use crate::{
    error::AppError,
    pdf::{BatchRenderRequest, PdfContext},
};

pub const CRATE_INFO: &str = concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"));

mod assets;
mod error;
pub mod logging;
pub mod pdf;
pub mod zip;

#[cfg(feature = "server")]
pub mod handlers;

#[cfg(all(test, feature = "server"))]
mod tests;

#[cfg(feature = "server")]
pub use server::start_server;

#[cfg(feature = "server")]
mod server {
    use std::{io, sync::Arc};

    use axum::{
        Router,
        routing::{get, post},
    };
    use tokio::net::TcpListener;
    use tracing::info;

    use crate::{error::AppError, handlers, pdf::PdfContext};

    /// Launch the HTTP server and publish the PDF rendering endpoint.
    pub async fn start_server(
        listener: TcpListener,
        pdf_context: PdfContext,
    ) -> Result<(), AppError> {
        let pdf_context = Arc::new(pdf_context);
        let router = Router::new()
            .route("/", get(handlers::root))
            .route(
                "/render-pdf/{template}/{file_name}",
                get(handlers::render_pdf),
            )
            .route("/render-pdf/batch", post(handlers::render_pdf_batch))
            .with_state(pdf_context);

        info!("HTTP listener ready; serving requests");
        if let Err(error) = axum::serve(listener, router).await {
            tracing::error!(%error, "Server encountered an error");
            return Err(io::Error::other(error).into());
        }

        Ok(())
    }
}
