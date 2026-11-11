//! Attachment text extraction: PDF and DOCX, plus draft derivation from plain text.

use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;

use super::content::ResumeContent;

pub const MAX_EXTRACTED_TEXT_BYTES: usize = 512 * 1024;
pub const EXTRACT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// The attachment formats this build knows how to parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentKind {
    Pdf,
    Docx,
}

impl AttachmentKind {
    pub fn mime(self) -> &'static str {
        match self {
            AttachmentKind::Pdf => super::model::PDF_MIME,
            AttachmentKind::Docx => super::model::DOCX_MIME,
        }
    }

    /// Canonical lowercased extension (without the dot) for this kind.
    pub fn extension(self) -> &'static str {
        match self {
            AttachmentKind::Pdf => "pdf",
            AttachmentKind::Docx => "docx",
        }
    }
}

/// Errors raised while turning an attachment into plain text.
#[derive(Debug, Error)]
pub enum ParseError {
    /// The byte content does not look like any supported format.
    #[error("attachment is not a supported document format")]
    UnsupportedFormat,
    /// Extraction exceeded the configured timeout.
    #[error("text extraction timed out")]
    Timeout,
    /// The document parsed successfully but contained no usable text.
    #[error("document contains no extractable text")]
    EmptyText,
    /// The document is malformed or unreadable.
    #[error("document could not be parsed: {message}")]
    Corrupt {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

impl ParseError {
    pub fn code(&self) -> super::model::ParseErrorCode {
        match self {
            ParseError::UnsupportedFormat => super::model::ParseErrorCode::UnsupportedFormat,
            ParseError::Timeout => super::model::ParseErrorCode::Timeout,
            ParseError::EmptyText => super::model::ParseErrorCode::EmptyText,
            ParseError::Corrupt { .. } => super::model::ParseErrorCode::Corrupt,
        }
    }
}

/// Extracts plain text from supported attachment kinds.
pub trait ResumeTextExtractor: Send + Sync {
    /// Whether this extractor can handle the given kind.
    fn supports(&self, kind: AttachmentKind) -> bool;

    /// Extract plain text from the raw bytes of the attachment.
    ///
    /// Implementations must not include the raw bytes in errors (they may embed
    /// the bytes in the payload they are given, but error messages must only
    /// carry parser messages).
    fn extract(&self, kind: AttachmentKind, bytes: &[u8]) -> Result<String, ParseError>;
}

/// PDF extractor backed by `pdf-extract`.
#[derive(Debug, Default, Clone, Copy)]
pub struct PdfTextExtractor;

#[async_trait]
impl ResumeTextExtractor for PdfTextExtractor {
    fn supports(&self, kind: AttachmentKind) -> bool {
        kind == AttachmentKind::Pdf
    }

    fn extract(&self, _kind: AttachmentKind, bytes: &[u8]) -> Result<String, ParseError> {
        let text =
            pdf_extract::extract_text_from_mem(bytes).map_err(|err| ParseError::Corrupt {
                message: "pdf extraction failed".to_string(),
                source: Some(Box::new(err)),
            })?;
        Ok(text)
    }
}

/// DOCX extractor backed by `docx-rs`.
#[derive(Debug, Default, Clone, Copy)]
pub struct DocxTextExtractor;

#[async_trait]
impl ResumeTextExtractor for DocxTextExtractor {
    fn supports(&self, kind: AttachmentKind) -> bool {
        kind == AttachmentKind::Docx
    }

    fn extract(&self, _kind: AttachmentKind, bytes: &[u8]) -> Result<String, ParseError> {
        let doc = docx_rs::read_docx(bytes).map_err(|err| ParseError::Corrupt {
            message: "docx parsing failed".to_string(),
            source: Some(Box::new(err)),
        })?;
        let mut out = String::new();
        collect_document_children(&doc.document.children, &mut out);
        Ok(out)
    }
}

fn collect_document_children(children: &[docx_rs::DocumentChild], out: &mut String) {
    for child in children {
        match child {
            docx_rs::DocumentChild::Paragraph(paragraph) => {
                collect_paragraph(paragraph, out);
                out.push('\n');
            }
            docx_rs::DocumentChild::Table(table) => {
                for docx_rs::TableChild::TableRow(row) in &table.rows {
                    for docx_rs::TableRowChild::TableCell(cell) in &row.cells {
                        collect_table_cell(cell, out);
                    }
                }
            }
            _ => {}
        }
    }
}

fn collect_table_cell(cell: &docx_rs::TableCell, out: &mut String) {
    for content in &cell.children {
        match content {
            docx_rs::TableCellContent::Paragraph(paragraph) => {
                collect_paragraph(paragraph, out);
                out.push('\n');
            }
            docx_rs::TableCellContent::Table(table) => {
                for docx_rs::TableChild::TableRow(row) in &table.rows {
                    for docx_rs::TableRowChild::TableCell(inner) in &row.cells {
                        collect_table_cell(inner, out);
                    }
                }
            }
            _ => {}
        }
    }
}

fn collect_paragraph(paragraph: &docx_rs::Paragraph, out: &mut String) {
    for child in &paragraph.children {
        if let docx_rs::ParagraphChild::Run(run) = child {
            for run_child in &run.children {
                if let docx_rs::RunChild::Text(text) = run_child {
                    out.push_str(&text.text);
                }
            }
        }
    }
}

/// Tries each extractor in order until one supports the kind.
#[derive(Default)]
pub struct CompositeTextExtractor {
    extractors: Vec<Arc<dyn ResumeTextExtractor>>,
}

impl std::fmt::Debug for CompositeTextExtractor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompositeTextExtractor")
            .field("extractors", &self.extractors.len())
            .finish()
    }
}

impl CompositeTextExtractor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(mut self, extractor: Arc<dyn ResumeTextExtractor>) -> Self {
        self.extractors.push(extractor);
        self
    }
}

#[async_trait]
impl ResumeTextExtractor for CompositeTextExtractor {
    fn supports(&self, kind: AttachmentKind) -> bool {
        self.extractors.iter().any(|e| e.supports(kind))
    }

    fn extract(&self, kind: AttachmentKind, bytes: &[u8]) -> Result<String, ParseError> {
        for extractor in &self.extractors {
            if extractor.supports(kind) {
                return extractor.extract(kind, bytes);
            }
        }
        Err(ParseError::UnsupportedFormat)
    }
}

/// Truncates a string to at most `max_bytes` UTF-8 bytes without splitting a
/// character. Returns the original string when it already fits.
pub fn truncate_utf8(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

/// Derives a conservative online draft from extracted plain text.
///
/// Only the summary is populated (first `MAX_SUMMARY_CHARS` characters of the
/// trimmed text; char-based like all content bounds). The extractor never
/// guesses structured fields, and the raw text itself is never persisted or
/// logged.
pub fn draft_from_text(text: &str) -> ResumeContent {
    let summary: String = text
        .trim()
        .chars()
        .take(super::content::MAX_SUMMARY_CHARS)
        .collect();
    ResumeContent {
        summary,
        ..ResumeContent::default()
    }
}
