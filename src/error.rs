use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use std::io;
use thiserror::Error;
use tokio::task::JoinError;
use tracing::error;
use typst::diag::SourceDiagnostic;
use uuid::Uuid;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("failed to canonicalize path `{path}`: {source}")]
    CanonicalizePath {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("path is not a directory: {0}")]
    NotADirectory(String),
    #[error("failed to serialize input JSON: {0}")]
    InputSerialization(#[from] serde_json::Error),
    #[error("main source `{0}` not found")]
    MainSourceNotFound(String),
    #[error("Typst compilation failed: {0:#?}")]
    TypstCompilation(Vec<SourceDiagnostic>),
    #[error("PDF export failed: {0:#?}")]
    PdfExport(Vec<SourceDiagnostic>),
    #[error("Background task failed to complete: {0}")]
    TaskJoin(#[from] JoinError),
    /// The client closed the connection before the ZIP archive was fully written.
    #[error("Client closed connection before ZIP archive completed")]
    ConnectionClosed,
    /// An error bubbled up from the underlying ZIP writer.
    #[error("ZIP writer error: {0}")]
    ZipError(#[from] async_zip::error::ZipError),
}

impl AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            AppError::MainSourceNotFound(_) => StatusCode::NOT_FOUND,
            AppError::TypstCompilation(_)
            | AppError::CanonicalizePath { .. }
            | AppError::NotADirectory(_)
            | AppError::ConnectionClosed
            | AppError::InputSerialization(_) => StatusCode::BAD_REQUEST,
            AppError::Io(_)
            | AppError::PdfExport(_)
            | AppError::TaskJoin(_)
            | AppError::ZipError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn public_message(&self) -> &'static str {
        match self {
            AppError::Io(_) => "I/O operation failed",
            AppError::CanonicalizePath { .. } => "Failed to resolve file path",
            AppError::NotADirectory(_) => "Provided path is not a directory",
            AppError::InputSerialization(_) => "Invalid request payload",
            AppError::MainSourceNotFound(_) => "Requested template not found",
            AppError::TypstCompilation(_) => "Document compilation failed",
            AppError::PdfExport(_) => "PDF export failed",
            AppError::TaskJoin(_) => "Worker task failed to complete",
            AppError::ConnectionClosed => "Client closed connection",
            AppError::ZipError(_) => "Failed to stream ZIP archive",
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let reference = Uuid::new_v4();
        error!(%reference, error = ?self, "Application error encountered");
        let body = json!({
            "error": self.public_message(),
            "reference": reference.to_string(),
        });
        (status, Json(body)).into_response()
    }
}
