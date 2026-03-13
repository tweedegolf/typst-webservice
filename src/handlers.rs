use std::{collections::HashSet, sync::Arc};

use axum::{
    Json,
    extract::{Path, State},
    http::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE},
    response::IntoResponse,
};
use axum_extra::response::Attachment;
use tokio::task::JoinSet;
use tracing::{debug, info, instrument};

use crate::{
    CRATE_INFO,
    error::AppError,
    pdf::PdfContext,
    zip::{ZipResponse, ZipResponseWriter},
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

/// Batch request configuration for PDF rendering.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct BatchRenderRequest {
    /// Name of the Typst template to render.
    template: String,
    /// File name (including extension) for the PDF inside the archive.
    file_name: String,
    /// JSON payload injected into the Typst template.
    input: serde_json::Value,
}

/// Render multiple Typst templates and stream the PDFs as a ZIP archive.
#[instrument(skip(pdf_context, requests))]
pub(crate) async fn render_pdf_batch(
    State(pdf_context): State<Arc<PdfContext>>,
    Json(requests): Json<Vec<BatchRenderRequest>>,
) -> Result<impl IntoResponse, AppError> {
    info!(count = requests.len(), "Received batch PDF render request");

    let (response, writer) = ZipResponse::new();
    let context = Arc::clone(&pdf_context);

    validate_batch_templates(pdf_context.as_ref(), &requests)?;
    spawn_batch_render(context, requests, writer);

    Ok(Attachment::new(response.into_body())
        .filename(BATCH_ARCHIVE_NAME)
        .content_type("application/zip"))
}

/// Ensure each batch request references an existing template before rendering.
fn validate_batch_templates(
    context: &PdfContext,
    requests: &[BatchRenderRequest],
) -> Result<(), AppError> {
    let mut checked_templates = HashSet::new();

    for request in requests {
        if checked_templates.insert(request.template.as_str())
            && !context.has_template(&request.template)
        {
            return Err(AppError::MainSourceNotFound(request.template.clone()));
        }
    }

    Ok(())
}

/// Start an asynchronous task that renders each batch entry into the streaming ZIP.
fn spawn_batch_render(
    context: Arc<PdfContext>,
    requests: Vec<BatchRenderRequest>,
    writer: ZipResponseWriter,
) {
    tokio::spawn(async move {
        if let Err(error) = write_batch_to_zip(context, requests, writer).await {
            tracing::error!(?error, "Failed to stream ZIP batch response");
        }
    });
}

/// Render each template in the batch and add the resulting PDFs to the ZIP archive.
#[instrument(skip(context, requests, writer))]
async fn write_batch_to_zip(
    context: Arc<PdfContext>,
    requests: Vec<BatchRenderRequest>,
    mut writer: ZipResponseWriter,
) -> Result<(), AppError> {
    let mut join_set = JoinSet::new();

    for request in requests {
        let BatchRenderRequest {
            template,
            file_name,
            input,
        } = request;

        let render_context = context.clone();
        join_set.spawn_blocking(move || {
            PdfContext::render(render_context, template, input)
                .map(|pdf_bytes| (file_name, pdf_bytes))
        });
    }

    while let Some(result) = join_set.join_next().await {
        let (file_name, pdf_bytes) = result??;
        writer.add_file(&file_name, &pdf_bytes).await?;
    }

    writer.finish().await?;

    Ok(())
}
