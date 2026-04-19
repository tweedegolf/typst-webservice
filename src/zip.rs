use async_zip::{Compression, ZipDateTime, ZipEntryBuilder, tokio::write::ZipFileWriter};
use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::error::AppError;

/// Writer used to add files into a streaming ZIP archive.
pub struct ZipResponseWriter<W: AsyncWrite + Unpin> {
    inner: ZipFileWriter<W>,
}

impl<W: AsyncWrite + Unpin> ZipResponseWriter<W> {
    /// Create a new writer wrapping the provided async writer.
    pub fn new(writer: W) -> Self {
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
    pub async fn finish(self) -> Result<W, AppError> {
        let mut writer = self.inner.close().await?.into_inner();
        writer
            .shutdown()
            .await
            .map_err(|_| AppError::ConnectionClosed)?;
        Ok(writer)
    }
}

#[cfg(feature = "server")]
pub use server::ZipResponse;

#[cfg(feature = "server")]
mod server {
    use axum::{
        body::Body,
        http::{StatusCode, header},
        response::{IntoResponse, Response},
    };
    use tokio::io::DuplexStream;
    use tokio_util::io::ReaderStream;

    use super::ZipResponseWriter;

    /// A ZIP file response, that streams its contents to the client every time a file is added.
    pub struct ZipResponse {
        inner: ReaderStream<DuplexStream>,
    }

    impl ZipResponse {
        /// Create a new [`ZipResponse`] with a default buffer size of 16KB.
        pub fn new() -> (Self, ZipResponseWriter<DuplexStream>) {
            Self::new_with_buffer_size(16 * 1024)
        }

        /// Create a new [`ZipResponse`] with a custom buffer size for the
        /// underlying channel.
        pub fn new_with_buffer_size(buffer_size: usize) -> (Self, ZipResponseWriter<DuplexStream>) {
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
}
