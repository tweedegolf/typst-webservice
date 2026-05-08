use std::sync::Arc;

use axum::{
    Json,
    body::Body,
    extract::{Path, State},
    http::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE},
    response::IntoResponse,
};
use axum_extra::response::Attachment;
use tracing::{debug, info, instrument};

use crate::{
    CRATE_INFO,
    error::AppError,
    pdf::{BatchRenderRequest, PdfContext},
};

const BATCH_ARCHIVE_NAME: &str = "rendered-pdfs.zip";

/// Report the running crate name and version.
pub(crate) async fn root(State(pdf_context): State<Arc<PdfContext>>) -> String {
    let templates = pdf_context.template_names();

    if templates.is_empty() {
        return format!("{CRATE_INFO}\n\nTemplates:\n(none)");
    }

    format!("{CRATE_INFO}\n\nTemplates:\n{}", templates.join("\n"))
}

/// Render a Typst template into a PDF and stream it back to the client.
#[instrument(skip(pdf_context, input), fields(template = %template, file_name = %file_name))]
pub(crate) async fn render_pdf(
    State(pdf_context): State<Arc<PdfContext>>,
    Path((template, file_name)): Path<(String, String)>,
    Json(input): Json<serde_json::Value>,
) -> Result<impl IntoResponse, AppError> {
    info!(%template, %file_name, "Received PDF render request");
    let pdf_bytes = PdfContext::render(pdf_context, template, input)?;
    debug!("Successfully rendered PDF ({} bytes)", pdf_bytes.len());

    Ok((
        [
            (CONTENT_TYPE, "application/pdf".to_string()),
            (CONTENT_LENGTH, pdf_bytes.len().to_string()),
            (
                CONTENT_DISPOSITION,
                format!("attachment; filename=\"{file_name}\""),
            ),
        ],
        pdf_bytes,
    ))
}

/// Render multiple Typst templates and stream the PDFs as a ZIP archive.
#[instrument(skip(pdf_context, requests))]
pub(crate) async fn render_pdf_batch(
    State(pdf_context): State<Arc<PdfContext>>,
    Json(requests): Json<Vec<BatchRenderRequest>>,
) -> Result<impl IntoResponse, AppError> {
    info!(count = requests.len(), "Received batch PDF render request");

    let stream = PdfContext::render_batch(pdf_context, requests)?;

    Ok(Attachment::new(Body::from_stream(stream))
        .filename(BATCH_ARCHIVE_NAME)
        .content_type("application/zip"))
}
