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
