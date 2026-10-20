//! Text of a PDF statement with its positions, read with PDFium.

use std::path::Path;
use std::sync::{Mutex, OnceLock};

use pdfium_render::prelude::*;

use super::PdfError;

/// A run of text on a line. Positions are in PDF points from the left of the page.
#[derive(Debug, Clone, PartialEq)]
pub struct TextItem {
    pub left: f32,
    pub right: f32,
    pub text: String,
}

/// Text items sharing a baseline, from left to right.
#[derive(Debug, Clone, PartialEq)]
pub struct TextLine {
    /// Middle of the line, in points from the top of the page.
    pub y: f32,
    pub items: Vec<TextItem>,
}

impl TextLine {
    /// Items joined with single spaces.
    pub fn text(&self) -> String {
        self.items
            .iter()
            .map(|item| item.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PageText {
    pub width: f32,
    pub height: f32,
    /// Lines from the top to the bottom of the page.
    pub lines: Vec<TextLine>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DocumentText {
    pub pages: Vec<PageText>,
}

impl DocumentText {
    /// Every line of every page, in reading order.
    pub fn lines(&self) -> impl Iterator<Item = &TextLine> {
        self.pages.iter().flat_map(|page| &page.lines)
    }

    /// Whether a line contains `needle`, to recognize the bank of a statement.
    pub fn contains(&self, needle: &str) -> bool {
        self.lines().any(|line| line.text().contains(needle))
    }
}

/// The single PDFium instance: the library can only be bound once per process.
static PDFIUM: OnceLock<Pdfium> = OnceLock::new();
static PDFIUM_INIT: Mutex<()> = Mutex::new(());

/// PDFium, loaded on first use from `library_dir` (`libpdfium.so` or `pdfium.dll`).
fn pdfium(library_dir: &Path) -> Result<&'static Pdfium, PdfError> {
    if let Some(pdfium) = PDFIUM.get() {
        return Ok(pdfium);
    }
    let _guard = PDFIUM_INIT
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(pdfium) = PDFIUM.get() {
        return Ok(pdfium);
    }
    let path = Pdfium::pdfium_platform_library_name_at_path(library_dir);
    let bindings = Pdfium::bind_to_library(&path)
        .map_err(|error| PdfError::LibraryUnavailable(format!("{}: {error}", path.display())))?;
    Ok(PDFIUM.get_or_init(|| Pdfium::new(bindings)))
}

/// Reads the text of a PDF with the position of each run of text.
pub fn extract(library_dir: &Path, bytes: &[u8]) -> Result<DocumentText, PdfError> {
    let pdfium = pdfium(library_dir)?;
    let document = pdfium
        .load_pdf_from_byte_slice(bytes, None)
        .map_err(|error| PdfError::InvalidPdf(error.to_string()))?;

    let mut pages = Vec::new();
    for page in document.pages().iter() {
        let height = page.height().value;
        let text = page
            .text()
            .map_err(|error| PdfError::InvalidPdf(error.to_string()))?;
        let segments = text
            .segments()
            .iter()
            .filter_map(|segment| {
                let bounds = segment.bounds();
                let content = segment.text().trim().to_owned();
                (!content.is_empty()).then(|| Segment {
                    top: height - bounds.top().value,
                    bottom: height - bounds.bottom().value,
                    left: bounds.left().value,
                    right: bounds.right().value,
                    text: content,
                })
            })
            .collect();
        pages.push(PageText {
            width: page.width().value,
            height,
            lines: group_lines(segments),
        });
    }
    Ok(DocumentText { pages })
}

/// A run of text with its box, in points from the top-left corner of the page.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Segment {
    pub top: f32,
    pub bottom: f32,
    pub left: f32,
    pub right: f32,
    pub text: String,
}

/// Groups segments into lines: a segment whose vertical middle falls within the height of a
/// line belongs to it. Lines are sorted from top to bottom, items from left to right.
pub(crate) fn group_lines(mut segments: Vec<Segment>) -> Vec<TextLine> {
    let middle = |s: &Segment| (s.top + s.bottom) / 2.0;
    segments.sort_by(|a, b| middle(a).total_cmp(&middle(b)));

    // (top, bottom, line) of the lines being built.
    let mut lines: Vec<(f32, f32, TextLine)> = Vec::new();
    for segment in segments {
        let y = middle(&segment);
        let item = TextItem {
            left: segment.left,
            right: segment.right,
            text: segment.text,
        };
        match lines.last_mut() {
            Some((top, bottom, line)) if y >= *top && y <= *bottom => line.items.push(item),
            _ => lines.push((
                segment.top,
                segment.bottom,
                TextLine {
                    y,
                    items: vec![item],
                },
            )),
        }
    }
    lines
        .into_iter()
        .map(|(_, _, mut line)| {
            line.items.sort_by(|a, b| a.left.total_cmp(&b.left));
            line
        })
        .collect()
}
