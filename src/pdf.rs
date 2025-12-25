use chrono::{Datelike, Timelike};
use std::{collections::HashMap, path::Path, sync::Arc, time::Instant};
use tracing::{debug, info, trace, warn};
use typst::{
    Library, LibraryExt, World,
    diag::{FileError, FileResult},
    foundations::{Bytes, Datetime},
    syntax::{FileId, Source, VirtualPath},
    text::{Font, FontBook},
    utils::LazyHash,
};

use crate::{
    assets::collect_dir_contents,
    error::{AppError, AppResult},
};

/// Shared Typst compilation state used when rendering PDFs.
pub struct PdfContext {
    sources: Vec<Source>,
    library: LazyHash<Library>,
    fontbook: LazyHash<FontBook>,
    assets: HashMap<FileId, Bytes>,
    fonts: Vec<Font>,
}

/// Wrapper implementing Typst's [`World`] trait for a single render invocation.
struct RenderInput {
    context: Arc<PdfContext>,
    main_source: Source,
    input_data: (FileId, Bytes),
}

impl RenderInput {
    /// Build a new render input for the requested template and JSON data.
    fn new(
        context: Arc<PdfContext>,
        source_name: String,
        input: serde_json::Value,
    ) -> AppResult<Self> {
        trace!(template = %source_name, "Preparing render input");
        // Find the main source by name
        let main_source = context
            .sources
            .iter()
            .find(|s| s.id().vpath().as_rootless_path().file_name() == Some(source_name.as_ref()))
            .cloned()
            .ok_or_else(|| AppError::MainSourceNotFound(source_name.clone()))?;
        trace!(template = %source_name, source_id = ?main_source.id(), "Resolved template source");

        // Prepare the input data as a virtual file
        let input_bytes = Bytes::new(serde_json::to_vec(&input)?);
        let input_file_id = FileId::new(None, VirtualPath::new(Path::new("input.json")));
        trace!(file_id = ?input_file_id, "Encoded render input as virtual file");

        Ok(RenderInput {
            context,
            main_source,
            input_data: (input_file_id, input_bytes),
        })
    }
}

impl PdfContext {
    /// Load all Typst sources, assets, and fonts from a directory tree into memory.
    pub fn from_directory(path: impl AsRef<Path>) -> AppResult<PdfContext> {
        let path = path.as_ref();
        let absolute_path =
            std::fs::canonicalize(path).map_err(|source| AppError::CanonicalizePath {
                path: path.display().to_string(),
                source,
            })?;

        info!("Loading assets from directory: {}", absolute_path.display());

        if !absolute_path.is_dir() {
            return Err(AppError::NotADirectory(absolute_path.display().to_string()));
        }

        let assets = collect_dir_contents(absolute_path)?;
        debug!(
            sources = assets.sources.len(),
            fonts = assets.fonts.len(),
            binaries = assets.assets.len(),
            "Collected assets from disk"
        );

        let mut fontbook = FontBook::new();
        for font in &assets.fonts {
            fontbook.push(font.info().clone());
        }

        Ok(PdfContext {
            sources: assets.sources,
            library: LazyHash::new(Library::default()),
            fontbook: LazyHash::new(fontbook),
            assets: assets.assets,
            fonts: assets.fonts,
        })
    }

    /// Check whether a template with the provided name exists in the context.
    pub fn has_template(&self, source_name: &str) -> bool {
        self.sources.iter().any(|source| {
            source
                .id()
                .vpath()
                .as_rootless_path()
                .file_name()
                .and_then(|name| name.to_str())
                == Some(source_name)
        })
    }

    /// Render a Typst template with the provided JSON payload into PDF bytes.
    pub fn render(
        context: Arc<Self>,
        source_name: String,
        input: serde_json::Value,
    ) -> AppResult<Vec<u8>> {
        trace!(template = %source_name, "Starting render pipeline");
        let render_input: RenderInput = RenderInput::new(context, source_name, input)?;

        let compile_start = Instant::now();
        let result = typst::compile(&render_input);
        let document = result
            .output
            .map_err(|errors| AppError::TypstCompilation(errors.into_iter().collect()))?;

        info!(
            "Compile took {} ms, {} warnings",
            compile_start.elapsed().as_millis(),
            result.warnings.len()
        );

        result.warnings.iter().for_each(|warning| {
            warn!("Warning: {:?}", warning);
            trace!(?warning, "Forwarded compile warning");
        });

        let pdf_gen_start = Instant::now();
        let pdf_bytes = typst_pdf::pdf(&document, &Default::default())
            .map_err(|errors| AppError::PdfExport(errors.into_iter().collect()))?;

        debug!(
            "PDF generation took {} ms",
            pdf_gen_start.elapsed().as_millis()
        );

        Ok(pdf_bytes)
    }
}

impl World for RenderInput {
    /// Provide access to the preloaded Typst standard library.
    fn library(&self) -> &LazyHash<Library> {
        &self.context.library
    }

    /// Expose the available fonts to the compiler.
    fn book(&self) -> &LazyHash<FontBook> {
        &self.context.fontbook
    }

    /// Identify the main source file for compilation.
    fn main(&self) -> FileId {
        self.main_source.id()
    }

    /// Retrieve a Typst source by its ID or report a missing file error.
    fn source(&self, id: FileId) -> FileResult<Source> {
        for source in &self.context.sources {
            if source.id() == id {
                trace!(?id, "Resolved source file");
                return Ok(source.clone());
            }
        }

        trace!(?id, "Source file not found");
        Err(FileError::NotFound(id.vpath().as_rootless_path().into()))
    }

    /// Retrieve a binary asset by its ID, including the injected JSON input.
    fn file(&self, id: FileId) -> FileResult<Bytes> {
        // if the file we need is the input file, pass that
        if self.input_data.0 == id {
            trace!(?id, "Served synthetic JSON input");
            return Ok(self.input_data.1.clone());
        }

        // otherwise it must be one of the other files
        self.context
            .assets
            .get(&id)
            .cloned()
            .inspect(|_| {
                trace!(?id, "Served binary asset");
            })
            .ok_or_else(|| {
                trace!(?id, "Binary asset not found");
                FileError::NotFound(id.vpath().as_rootless_path().into())
            })
    }

    /// Return a font from the context by index, if present.
    fn font(&self, index: usize) -> Option<Font> {
        self.context.fonts.get(index).cloned()
    }

    /// Provide the current date, optionally offset by hours, to the document.
    fn today(&self, offset: Option<i64>) -> Option<Datetime> {
        let datetime = match offset {
            Some(offset) => chrono::Utc::now() + chrono::Duration::hours(offset),
            None => chrono::Utc::now(),
        };
        trace!(?offset, ?datetime, "Providing current datetime");

        Datetime::from_ymd_hms(
            datetime.year(),
            datetime.month() as u8,
            datetime.day() as u8,
            datetime.hour() as u8,
            datetime.minute() as u8,
            datetime.second() as u8,
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use uuid::Uuid;

    /// Verify that rendering inserts dynamic data into the resulting PDF.
    #[test]
    fn test_pdf_generation() {
        crate::logging::init_for_tests();
        let context = PdfContext::from_directory("./assets").unwrap();
        let name = Uuid::new_v4().to_string();
        let pdf_bytes = PdfContext::render(
            Arc::new(context),
            "example.typ".to_string(),
            serde_json::json!({
                "name": name,
                "list": ["Memory Safety", "Open Source", "World Peace"]
            }),
        )
        .unwrap();

        // write to disk
        std::fs::write("test_output.pdf", &pdf_bytes).unwrap();

        assert!(!pdf_bytes.is_empty(), "expected PDF body to be non-empty");
        let pdf_text = String::from_utf8_lossy(&pdf_bytes);
        assert!(
            pdf_text.contains(&name),
            "expected generated PDF to contain the dynamic name"
        );
    }
}
