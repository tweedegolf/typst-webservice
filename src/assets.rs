use std::{collections::HashMap, fs, path::Path};

use typst::{
    foundations::Bytes,
    syntax::{FileId, Source, VirtualPath},
    text::Font,
};

use crate::error::AppResult;
use tracing::{debug, trace};

/// Represents the type of a file based on its extension.
#[derive(Debug)]
enum FileType {
    Font,
    TypstSource,
    Other,
}

impl FileType {
    /// Determine the [`FileType`] for the provided path by checking the extension.
    fn from_path(path: impl AsRef<Path>) -> Self {
        let ext = path
            .as_ref()
            .extension()
            .map(|ext| ext.to_str().unwrap_or_default().to_lowercase());

        match ext.as_deref() {
            None => FileType::Other,
            Some("ttf") | Some("otf") | Some("woff") | Some("woff2") => FileType::Font,
            Some("typ") | Some("typst") => FileType::TypstSource,
            _ => FileType::Other,
        }
    }
}

/// Aggregated Typst sources, binary assets, and fonts discovered on disk.
#[derive(Default)]
pub struct Assets {
    pub sources: Vec<Source>,
    pub assets: HashMap<FileId, Bytes>,
    pub fonts: Vec<Font>,
}

impl Assets {
    /// Merge another [`Assets`] collection into this one.
    fn merge(&mut self, other: Assets) {
        self.sources.extend(other.sources);
        self.assets.extend(other.assets);
        self.fonts.extend(other.fonts);
    }

    /// Insert a file into the collection based on its detected [`FileType`].
    fn add_file(&mut self, path: &Path, relative_path: &Path) -> AppResult<()> {
        let file_type = FileType::from_path(path);
        trace!(
            absolute = %path.display(),
            relative = %relative_path.display(),
            ?file_type,
            "Processing asset file"
        );

        match file_type {
            FileType::TypstSource => {
                let content = fs::read_to_string(path)?;
                let file_id = FileId::new(None, VirtualPath::new(relative_path));
                self.sources.push(Source::new(file_id, content));
                debug!(file = %relative_path.display(), "Loaded Typst source file");
            }
            FileType::Font => {
                let content = fs::read(path)?;
                if let Some(font) = Font::new(Bytes::new(content), 0) {
                    debug!(
                        file = %relative_path.display(),
                        family = %font.info().family,
                        "Loaded font file"
                    );
                    self.fonts.push(font);
                }
            }
            FileType::Other => {
                let content = fs::read(path)?;
                let file_id = FileId::new(None, VirtualPath::new(relative_path));
                self.assets.insert(file_id, Bytes::new(content));
                debug!(file = %relative_path.display(), "Loaded binary asset");
            }
        }

        Ok(())
    }
}

/// Recursively collect every asset/file within the provided directory tree.
pub fn collect_dir_contents(dir: impl AsRef<Path>) -> AppResult<Assets> {
    let dir = dir.as_ref();
    debug!(path = %dir.display(), "Scanning asset directory");
    let mut assets = Assets::default();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            trace!(path = %path.display(), "Descending into subdirectory");
            assets.merge(collect_dir_contents(&path)?);
        } else if path.is_file() {
            let relative_path = path.strip_prefix(dir).unwrap_or(&path);
            assets.add_file(&path, relative_path)?;
        }
    }

    Ok(assets)
}

#[cfg(test)]
mod tests {
    use super::collect_dir_contents;
    use std::path::Path;
    use typst::syntax::{FileId, VirtualPath};

    /// Ensure the assets directory exposes the expected sources, assets, and fonts.
    #[test]
    fn collect_dir_contents_includes_expected_assets() {
        crate::logging::init_for_tests();
        let assets = collect_dir_contents("./assets").expect("Failed to load assets directory");

        let has_example_source = assets
            .sources
            .iter()
            .any(|source| source.id().vpath().as_rootless_path() == Path::new("example.typ"));
        assert!(
            has_example_source,
            "expected example.typ to be loaded as a source"
        );

        let input_id = FileId::new(None, VirtualPath::new(Path::new("input.json")));
        assert!(
            assets.assets.contains_key(&input_id),
            "expected input.json to be present in miscellaneous assets"
        );

        let has_bagnard_font = assets
            .fonts
            .iter()
            .any(|font| font.info().family == "Bagnard");
        assert!(
            has_bagnard_font,
            "expected Bagnard.otf to be loaded into the font collection"
        );
    }
}
