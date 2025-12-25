use async_zip::{Compression, ZipDateTime, ZipEntryBuilder, tokio::write::ZipFileWriter};
use axum::{
    body::Body,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use tokio::io::{AsyncWriteExt, DuplexStream};
use tokio_util::io::ReaderStream;

use crate::error::AppError;

/// A ZIP file response, that streams its contents to the client every time a file is added
pub struct ZipResponse {
    inner: ReaderStream<DuplexStream>,
}

impl ZipResponse {
    /// Create a new [`ZipResponse`] with a default buffer size of 16KB.
    pub fn new() -> (Self, ZipResponseWriter) {
        Self::new_with_buffer_size(16 * 1024)
    }

    /// Create a new [`ZipResponse`] with a custom buffer size for the
    /// underlying channel.
    pub fn new_with_buffer_size(buffer_size: usize) -> (Self, ZipResponseWriter) {
        let (reader, writer) = tokio::io::duplex(buffer_size);

        (
            Self {
                inner: ReaderStream::new(reader),
            },
            ZipResponseWriter::new(writer),
        )
    }

    /// Convert this `ZipResponse` into an Axum [`Body`] stream.
    pub fn into_body(self) -> Body {
        Body::from_stream(self.inner)
    }
}

impl IntoResponse for ZipResponse {
    fn into_response(self) -> Response<Body> {
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/zip")],
            self.into_body(),
        )
            .into_response()
    }
}

/// Writer used to add files into a streaming ZIP archive.
pub struct ZipResponseWriter {
    inner: ZipFileWriter<DuplexStream>,
}

impl ZipResponseWriter {
    /// Create a new writer wrapping the provided duplex stream.
    fn new(writer: DuplexStream) -> Self {
        Self {
            inner: ZipFileWriter::with_tokio(writer),
        }
    }

    /// Add a file with the given name and contents to the archive.
    pub async fn add_file(&mut self, name: &str, data: &[u8]) -> Result<(), AppError> {
        let builder = ZipEntryBuilder::new(name.into(), Compression::Deflate)
            .last_modification_date(ZipDateTime::from_chrono(&chrono::Utc::now()));

        Ok(self.inner.write_entry_whole(builder, data).await?)
    }

    /// Finish writing the archive and flush the underlying stream.
    pub async fn finish(self) -> Result<(), AppError> {
        let final_writer = self.inner.close().await?;

        final_writer
            .into_inner()
            .shutdown()
            .await
            .map_err(|_| AppError::ConnectionClosed)?;

        Ok(())
    }
}
