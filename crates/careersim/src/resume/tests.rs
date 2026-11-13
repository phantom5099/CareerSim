//! resume 模块单元测试：内容边界、格式探测、文本提取与附件存储。

use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;

use super::content::{
    MAX_FIELD_CHARS, MAX_ITEMS, MAX_SERIALIZED_BYTES, MAX_SUMMARY_CHARS, ResumeContent, ResumeEducation,
    ResumeSkill,
};
use super::model::{MAX_ATTACHMENT_BYTES, ResumeError, validate_title};
use super::parse::{
    AttachmentKind, CompositeTextExtractor, DocxTextExtractor, PdfTextExtractor,
    ResumeTextExtractor, truncate_utf8,
};
use super::service::{detect_attachment_kind, sanitize_file_name};
use super::storage::{
    AttachmentStore, LocalFsAttachmentStore, read_attachment_bytes, validate_storage_key,
};
use super::structurize::structurize;

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
fn structurize_falls_back_to_summary_when_nothing_matches() {
    let draft = structurize("  第一行\n第二行  ");
    assert_eq!(draft.summary, "第一行\n第二行");
    assert!(draft.name.is_empty() && draft.experiences.is_empty());

    let long = "字".repeat(MAX_SUMMARY_CHARS + 50);
    let draft = structurize(&long);
    assert_eq!(draft.summary.chars().count(), MAX_SUMMARY_CHARS);
}

#[test]
fn structurize_extracts_contact_and_sections() {
    let text = "\
张三
13812345678
zhangsan@example.com
城市：上海

教育经历
2020.09-2024.06 复旦大学 本科 计算机科学与技术

实习经历
2023.06-2023.09 字节跳动 后端开发实习生
负责内容管理服务开发

专业技能
Rust、SQL、Kubernetes
";
    let parsed = structurize(text);
    assert_eq!(parsed.name, "张三");
    assert_eq!(parsed.phone, "13812345678");
    assert_eq!(parsed.email, "zhangsan@example.com");
    assert_eq!(parsed.city, "上海");

    assert_eq!(parsed.educations.len(), 1);
    let education = &parsed.educations[0];
    assert_eq!(education.school, "复旦大学");
    assert_eq!(education.degree, "本科");
    assert_eq!(education.major, "计算机科学与技术");
    assert_eq!(education.start_date, "2020.09");
    assert_eq!(education.end_date, "2024.06");

    assert_eq!(parsed.experiences.len(), 1);
    let experience = &parsed.experiences[0];
    assert_eq!(experience.company, "字节跳动");
    assert_eq!(experience.role, "后端开发实习生");
    assert_eq!(experience.kind, "实习");
    assert!(experience.description.contains("内容管理服务"));

    assert_eq!(parsed.skills.len(), 3);
    assert_eq!(parsed.skills[0].name, "Rust");
}

#[test]
fn structurize_parses_title_line_above_date_line() {
    // 两行式布局：标题行在前、日期行单独成行（旧解析器整条丢失）。
    let text = "\
张三
zhangsan@example.com

项目经历
电商推荐系统
2023.01-2023.06
- 负责召回模块开发

实习经历
腾讯 | 后端开发实习生
2023.07-2023.10
- 参与网关重构
";
    let parsed = structurize(text);
    assert_eq!(parsed.projects.len(), 1);
    assert_eq!(parsed.projects[0].name, "电商推荐系统");
    assert_eq!(parsed.projects[0].start_date, "2023.01");
    assert!(parsed.projects[0].description.contains("召回模块"));

    assert_eq!(parsed.experiences.len(), 1);
    assert_eq!(parsed.experiences[0].company, "腾讯");
    assert_eq!(parsed.experiences[0].role, "后端开发实习生");
    assert_eq!(parsed.experiences[0].kind, "实习");
}

#[test]
fn structurize_parses_company_line_then_role_with_date() {
    // 混合式布局：公司独行，职位与日期同行。
    let text = "\
工作经历
阿里巴巴集团
高级开发工程师 2020.03-2024.01
- 负责交易链路
";
    let parsed = structurize(text);
    assert_eq!(parsed.experiences.len(), 1);
    assert_eq!(parsed.experiences[0].company, "阿里巴巴集团");
    assert_eq!(parsed.experiences[0].role, "高级开发工程师");
    assert_eq!(parsed.experiences[0].kind, "工作");
    assert!(parsed.experiences[0].description.contains("交易链路"));
}

#[test]
fn structurize_education_backfills_school_and_gpa_from_description() {
    // 教育条目头只有日期与学校，学历/绩点在描述行。
    let text = "\
教育经历
2018.09-2022.06 浙江大学
计算机科学与技术 本科
GPA: 3.86/4.0
";
    let parsed = structurize(text);
    assert_eq!(parsed.educations.len(), 1);
    let education = &parsed.educations[0];
    assert_eq!(education.school, "浙江大学");
    assert!(education.degree.contains("本科"));
    assert!(education.ranking.contains("3.86"));
}

#[test]
fn merge_parsed_fills_only_empty_fields_and_dedupes() {
    use super::structurize::merge_parsed;

    let mut base = ResumeContent::default();
    base.name = "李四".to_owned();
    base.summary = "已有自我介绍".to_owned();
    let existing = ResumeEducation {
        school: "复旦大学".to_owned(),
        degree: "本科".to_owned(),
        major: String::new(),
        start_date: String::new(),
        end_date: String::new(),
        ranking: String::new(),
    };
    base.educations.push(existing.clone());

    let mut parsed = ResumeContent::default();
    parsed.name = "张三".to_owned();
    parsed.phone = "13812345678".to_owned();
    parsed.summary = "解析的自我介绍".to_owned();
    parsed.educations.push(existing);
    parsed.educations.push(ResumeEducation {
        school: "上海交通大学".to_owned(),
        degree: String::new(),
        major: String::new(),
        start_date: String::new(),
        end_date: String::new(),
        ranking: String::new(),
    });

    let merged = merge_parsed(&base, &parsed);
    // 已填字段不被覆盖。
    assert_eq!(merged.name, "李四");
    assert_eq!(merged.summary, "已有自我介绍");
    // 空字段被回填。
    assert_eq!(merged.phone, "13812345678");
    // 列表去重后追加。
    assert_eq!(merged.educations.len(), 2);
    assert_eq!(merged.educations[1].school, "上海交通大学");
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
