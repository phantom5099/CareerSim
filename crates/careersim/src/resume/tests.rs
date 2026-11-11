//! resume 模块单元测试：内容边界、格式探测、文本提取与附件存储。

use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;

use super::content::{
    MAX_FIELD_CHARS, MAX_ITEMS, MAX_SERIALIZED_BYTES, MAX_SUMMARY_CHARS, ResumeContent, ResumeSkill,
};
use super::model::{MAX_ATTACHMENT_BYTES, ResumeError, validate_title};
use super::parse::{
    AttachmentKind, CompositeTextExtractor, DocxTextExtractor, PdfTextExtractor,
    ResumeTextExtractor, draft_from_text, truncate_utf8,
};
use super::service::{detect_attachment_kind, sanitize_file_name};
use super::storage::{
    AttachmentStore, LocalFsAttachmentStore, read_attachment_bytes, validate_storage_key,
};

// ---------- 标题与内容边界 ----------

#[test]
fn title_is_trimmed_and_bounded() {
    assert_eq!(validate_title("  我的简历 ").unwrap(), "我的简历");
    assert!(matches!(
        validate_title("   "),
        Err(ResumeError::InvalidTitle { .. })
    ));
    let long = "长".repeat(81);
    assert!(matches!(
        validate_title(&long),
        Err(ResumeError::InvalidTitle { .. })
    ));
    let exactly_80 = "短".repeat(80);
    assert!(validate_title(&exactly_80).is_ok());
}

#[test]
fn content_rejects_overlong_field() {
    let content = ResumeContent {
        name: "名".repeat(MAX_FIELD_CHARS + 1),
        ..ResumeContent::default()
    };
    match content.validate() {
        Err(ResumeError::InvalidContent { field, .. }) => assert_eq!(field, "name"),
        other => panic!("expected InvalidContent, got {other:?}"),
    }
}

#[test]
fn content_rejects_too_many_items() {
    let content = ResumeContent {
        skills: (0..MAX_ITEMS + 1)
            .map(|index| ResumeSkill {
                name: format!("技能{index}"),
                level: "熟练".to_owned(),
            })
            .collect(),
        ..ResumeContent::default()
    };
    match content.validate() {
        Err(ResumeError::InvalidContent { field, .. }) => assert_eq!(field, "skills"),
        other => panic!("expected InvalidContent, got {other:?}"),
    }
}

#[test]
fn content_rejects_overlong_summary() {
    let content = ResumeContent {
        summary: "简".repeat(MAX_SUMMARY_CHARS + 1),
        ..ResumeContent::default()
    };
    match content.validate() {
        Err(ResumeError::InvalidContent { field, .. }) => assert_eq!(field, "summary"),
        other => panic!("expected InvalidContent, got {other:?}"),
    }
}

#[test]
fn content_to_json_rejects_oversized_payload() {
    let content = ResumeContent {
        // 远超 256 KiB
        summary: "长".repeat(MAX_SERIALIZED_BYTES),
        ..ResumeContent::default()
    };
    assert!(matches!(
        content.to_json(),
        Err(ResumeError::ContentTooLarge)
    ));
}

#[test]
fn content_serializes_camel_case_and_defaults() {
    let mut content = ResumeContent {
        name: "张三".to_owned(),
        ..ResumeContent::default()
    };
    content.skills.push(ResumeSkill {
        name: "Rust".to_owned(),
        level: "熟练".to_owned(),
    });
    let json = content.to_json().unwrap();
    assert_eq!(json["name"], "张三");
    assert_eq!(json["skills"][0]["name"], "Rust");
    // 数组缺省时反序列化为空集合；camelCase 键被接受。
    let parsed: ResumeContent =
        serde_json::from_value(serde_json::json!({ "name": "李四", "targetRole": "后端" }))
            .unwrap();
    assert_eq!(parsed.name, "李四");
    assert_eq!(parsed.target_role, "后端");
    assert!(parsed.educations.is_empty());
}

// ---------- 附件格式探测与文件名清洗 ----------

fn fake_pdf_bytes() -> Vec<u8> {
    let mut bytes = b"%PDF-1.4\n".to_vec();
    bytes.extend(std::iter::repeat_n(b'x', 512));
    bytes
}

fn pack_docx(paragraphs: &[&str]) -> Vec<u8> {
    let mut doc = docx_rs::Docx::new();
    for text in paragraphs {
        doc = doc
            .add_paragraph(docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text(*text)));
    }
    let mut cursor = Cursor::new(Vec::new());
    doc.pack(&mut cursor)
        .expect("pack docx document for testing");
    cursor.into_inner()
}

#[test]
fn detect_kind_requires_extension_and_magic_agreement() {
    let pdf = fake_pdf_bytes();
    let docx = pack_docx(&["hello"]);

    assert_eq!(
        detect_attachment_kind("resume.pdf", &pdf).unwrap(),
        AttachmentKind::Pdf
    );
    assert_eq!(
        detect_attachment_kind("resume.docx", &docx).unwrap(),
        AttachmentKind::Docx
    );

    // 扩展名与 magic bytes 不一致一律拒绝。
    assert!(matches!(
        detect_attachment_kind("resume.docx", &pdf),
        Err(ResumeError::UnsupportedAttachment)
    ));
    assert!(matches!(
        detect_attachment_kind("resume.pdf", &docx),
        Err(ResumeError::UnsupportedAttachment)
    ));
    // 纯文本冒充 PDF 拒绝。
    assert!(matches!(
        detect_attachment_kind("resume.pdf", b"plain text"),
        Err(ResumeError::UnsupportedAttachment)
    ));
    // 空文件拒绝。
    assert!(matches!(
        detect_attachment_kind("resume.pdf", b""),
        Err(ResumeError::UnsupportedAttachment)
    ));
    // 遗留 .doc 不支持。
    assert!(matches!(
        detect_attachment_kind("resume.doc", &pdf),
        Err(ResumeError::UnsupportedAttachment)
    ));
}

#[test]
fn sanitize_file_name_rules() {
    assert_eq!(
        sanitize_file_name("  我的简历.pdf ").unwrap(),
        "我的简历.pdf"
    );
    assert!(sanitize_file_name("   ").is_err());
    assert!(sanitize_file_name("a/b.pdf").is_err());
    assert!(sanitize_file_name("a\\b.pdf").is_err());
    assert!(sanitize_file_name("..hidden.pdf").is_err());
    assert!(sanitize_file_name("bad\nname.pdf").is_err());
    let long = "长".repeat(256);
    assert!(sanitize_file_name(&long).is_err());
}

// ---------- 文本提取 ----------

#[test]
fn truncate_utf8_never_splits_a_char() {
    assert_eq!(truncate_utf8("你好", 100), "你好");
    assert_eq!(truncate_utf8("你好", 4), "你");
    assert_eq!(truncate_utf8("你好", 2), "");
    assert_eq!(truncate_utf8("abc", 2), "ab");
}

#[test]
fn draft_from_text_only_fills_summary() {
    let draft = draft_from_text("  第一行\n第二行  ");
    assert_eq!(draft.summary, "第一行\n第二行");
    assert!(draft.name.is_empty() && draft.experiences.is_empty());

    let long = "字".repeat(MAX_SUMMARY_CHARS + 50);
    let draft = draft_from_text(&long);
    assert_eq!(draft.summary.chars().count(), MAX_SUMMARY_CHARS);
}

#[test]
fn docx_extractor_roundtrips_paragraphs() {
    let bytes = pack_docx(&["姓名：张三", "求职意向：后端开发"]);
    let extractor = DocxTextExtractor;
    assert!(extractor.supports(AttachmentKind::Docx));
    let text = extractor.extract(AttachmentKind::Docx, &bytes).unwrap();
    assert!(text.contains("姓名：张三"));
    assert!(text.contains("求职意向：后端开发"));
}

#[test]
fn docx_extractor_reads_table_cells() {
    let cell_paragraph =
        docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("表格技能"));
    let table = docx_rs::Table::new(vec![docx_rs::TableRow::new(vec![
        docx_rs::TableCell::new().add_paragraph(cell_paragraph),
    ])]);
    let doc = docx_rs::Docx::new().add_table(table);
    let mut cursor = Cursor::new(Vec::new());
    doc.pack(&mut cursor).expect("pack docx table");
    let text = DocxTextExtractor
        .extract(AttachmentKind::Docx, &cursor.into_inner())
        .unwrap();
    assert!(text.contains("表格技能"));
}

#[test]
fn pdf_extractor_rejects_garbage() {
    match PdfTextExtractor.extract(AttachmentKind::Pdf, b"this is not a pdf") {
        Err(super::parse::ParseError::Corrupt { .. }) => {}
        other => panic!("expected Corrupt, got {other:?}"),
    }
}

#[test]
fn composite_without_extractors_reports_unsupported() {
    let composite = CompositeTextExtractor::new();
    let err = composite
        .extract(AttachmentKind::Pdf, fake_pdf_bytes().as_slice())
        .unwrap_err();
    assert!(matches!(err, super::parse::ParseError::UnsupportedFormat));
}

// ---------- 存储键与本地文件系统适配器 ----------

#[test]
fn storage_key_validation_rejects_traversal() {
    assert!(validate_storage_key("0b7f9a1c-4f2d-4a5b-8c3d-1e2f3a4b5c6d").is_ok());
    assert!(validate_storage_key("").is_err());
    assert!(validate_storage_key("a/b").is_err());
    assert!(validate_storage_key("a\\b").is_err());
    assert!(validate_storage_key("..").is_err());
    assert!(validate_storage_key("a b").is_err());
    assert!(validate_storage_key("键名").is_err());
    assert!(validate_storage_key(&"a".repeat(129)).is_err());
}

fn temp_attachment_root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "careersim-resume-tests-{tag}-{}",
        uuid::Uuid::new_v4()
    ))
}

#[tokio::test]
async fn local_fs_store_put_open_delete_roundtrip() {
    let root = temp_attachment_root("roundtrip");
    let store = LocalFsAttachmentStore::new(&root);
    let key = uuid::Uuid::new_v4().to_string();

    store.put(&key, b"attachment bytes").await.unwrap();
    let reader = store.open(&key).await.unwrap();
    let bytes = read_attachment_bytes(reader).await.unwrap();
    assert_eq!(bytes, b"attachment bytes");

    store.delete(&key).await.unwrap();
    assert!(store.open(&key).await.is_err());
    // 幂等删除。
    store.delete(&key).await.unwrap();

    std::fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn read_attachment_bytes_enforces_limit() {
    let oversized = vec![0u8; MAX_ATTACHMENT_BYTES + 1];
    let reader = Box::new(Cursor::new(oversized));
    assert!(read_attachment_bytes(reader).await.is_err());

    let exact = vec![0u8; MAX_ATTACHMENT_BYTES];
    let reader = Box::new(Cursor::new(exact));
    assert!(read_attachment_bytes(reader).await.is_ok());
}

// ---------- 服务辅助函数 ----------

#[test]
fn attachment_kind_mime_values_are_fixed() {
    assert_eq!(AttachmentKind::Pdf.mime(), "application/pdf");
    assert_eq!(
        AttachmentKind::Docx.mime(),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
    );
}

#[test]
fn composite_extraction_dispatches_by_kind() {
    let composite = super::parse::CompositeTextExtractor::new()
        .push(Arc::new(PdfTextExtractor))
        .push(Arc::new(DocxTextExtractor));
    let docx = pack_docx(&["组合提取"]);
    let text = composite
        .extract(AttachmentKind::Docx, &docx)
        .expect("docx extraction through composite");
    assert!(text.contains("组合提取"));
}
