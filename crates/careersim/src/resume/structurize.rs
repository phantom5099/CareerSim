//! 规则化简历结构化：分段 + 正则 + 关键词词典（无模型、纯规则）。
//!
//! 管线：联系区正则提取（手机/邮箱/姓名/标签字段）→ 段头词典切分 →
//! 日期锚切条目 → 学历/公司/职位关键词映射 → 兜底 summary。
//! 规则解析对标准单栏排版命中率高，花式排版留空由人工补全。

use std::sync::OnceLock;

use regex::Regex;

use super::content::{
    MAX_FIELD_CHARS, MAX_ITEM_CHARS, MAX_ITEM_DESCRIPTION_CHARS, MAX_SUMMARY_CHARS, ResumeContent,
    ResumeCertificate, ResumeEducation, ResumeExperience, ResumeHonor, ResumeProject, ResumePractice,
    ResumeSkill,
};

/// 段类型；`Experience` 的布尔值标记是否实习段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Education,
    Experience { internship: bool },
    Projects,
    Practices,
    Skills,
    Certificates,
    Honors,
    Summary,
}

impl Section {
    fn match_header(line: &str) -> Option<Self> {
        let lower = line.to_ascii_lowercase();
        // 顺序敏感：具体词在前，泛词在后。
        const TABLE: &[(&[&str], Section)] = &[
            (
                &["实习经历", "实习经验", "实习实践", "internship"],
                Section::Experience { internship: true },
            ),
            (
                &[
                    "工作经历",
                    "工作经验",
                    "工作履历",
                    "职业经历",
                    "work experience",
                    "employment",
                    "experience",
                ],
                Section::Experience { internship: false },
            ),
            (
                &["项目经历", "项目经验", "项目实践", "projects"],
                Section::Projects,
            ),
            (
                &[
                    "校园经历",
                    "校园实践",
                    "在校经历",
                    "社会实践",
                    "实践活动",
                    "学生工作",
                    "志愿",
                ],
                Section::Practices,
            ),
            (
                &["教育经历", "教育背景", "学业经历", "education", "academic"],
                Section::Education,
            ),
            (&["专业技能", "技能特长", "技能", "skills"], Section::Skills),
            (
                &["资格证书", "证书", "certificates", "certifications"],
                Section::Certificates,
            ),
            (
                &["荣誉奖项", "荣誉", "奖项", "获奖", "awards", "honors"],
                Section::Honors,
            ),
            (
                &[
                    "自我评价",
                    "自我介绍",
                    "个人简介",
                    "个人评价",
                    "关于我",
                    "summary",
                    "objective",
                    "profile",
                    "about me",
                ],
                Section::Summary,
            ),
        ];
        for (keys, section) in TABLE {
            if keys.iter().any(|key| lower.contains(*key)) {
                return Some(*section);
            }
        }
        None
    }
}

/// 行是否像段头：短、无日期、去装饰后命中词典。
fn detect_section(raw_line: &str) -> Option<Section> {
    let line = raw_line.trim();
    if line.chars().count() > 30 || contains_date(line) {
        return None;
    }
    let stripped = strip_decorations(line);
    if stripped.is_empty() {
        return None;
    }
    Section::match_header(&stripped)
}

/// 去掉段头常见装饰：`【】`、`#`、`*`、`-`、`•`、序号（`一、` `1.` `1、`）与结尾冒号。
fn strip_decorations(line: &str) -> &str {
    let mut text = line.trim();
    for (open, close) in [("【", "】"), ("[", "]")] {
        if let Some(rest) = text.strip_prefix(open) {
            if let Some(inner) = rest.strip_suffix(close) {
                text = inner;
            }
        }
    }
    let mut text = text.trim_start_matches(['#', '*', '-', '•', '·', '—']);
    // 去掉"一、""1."、"1、"类序号前缀。
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }
    if index > 0 {
        let rest = &text[index..];
        if let Some(stripped) = rest.strip_prefix(['.', '、', ' ']) {
            text = stripped;
        }
    }
    for prefix in ["一、", "二、", "三、", "四、", "五、", "六、", "七、", "八、", "九、", "十、"] {
        if let Some(rest) = text.strip_prefix(prefix) {
            text = rest;
        }
    }
    text.trim_end_matches(['：', ':', '　'])
        .trim()
}

fn contains_date(text: &str) -> bool {
    date_token_positions(text).next().is_some()
}

/// 日期 token 的字节区间迭代器：`2020.09`、`2020-09`、`2020/09`、`2020年9月`、`2020`。
fn date_token_positions(text: &str) -> impl Iterator<Item = (usize, usize)> {
    static DATE_TOKEN: OnceLock<Regex> = OnceLock::new();
    let regex = DATE_TOKEN.get_or_init(|| {
        Regex::new(r"\d{4}\s*[.\-/年]\s*\d{1,2}\s*月?|\d{4}\s*年|\d{4}").expect("date token regex")
    });
    regex.find_iter(text).map(|m| (m.start(), m.end()))
}

/// 归一化日期 token 文本：去空白，统一连接符。
fn normalize_date(raw: &str) -> String {
    raw.chars().filter(|ch| !ch.is_whitespace()).collect()
}

/// 从一行里取日期区间；返回 `(start, end, 整段区间文本区间)`。
/// 支持 `2020.09-2024.06`、`2020.09 - 至今`、`2020-2024`。
fn extract_date_range(line: &str) -> Option<(String, String, usize, usize)> {
    static RANGE: OnceLock<Regex> = OnceLock::new();
    let regex = RANGE.get_or_init(|| {
        Regex::new(
            r"(?P<start>\d{4}\s*[.\-/年]\s*\d{1,2}\s*月?|\d{4})\s*(?:[-–—~至到]\s*(?P<end>至今|现在|now|present|\d{4}\s*[.\-/年]\s*\d{1,2}\s*月?|\d{4}))?",
        )
        .expect("date range regex")
    });
    let matched = regex.captures(line)?;
    let start_raw = matched.name("start")?.as_str();
    if !start_raw.chars().any(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let start = normalize_date(start_raw);
    let whole = matched.get(0)?;
    let end = match matched.name("end") {
        Some(end) => normalize_date(end.as_str()),
        // 单个年份没有区间终点时视为同一年（如 "2020" → 2020~2020）。
        None => start.clone(),
    };
    Some((start, end, whole.start(), whole.end()))
}

fn phone_regex() -> &'static Regex {
    static PHONE: OnceLock<Regex> = OnceLock::new();
    // regex crate 不支持 look-around；用捕获组隔离手机号本体（11 位，可带分隔符）。
    PHONE.get_or_init(|| {
        Regex::new(r"(?:^|\D)(1[3-9]\d[-\s]?\d{4}[-\s]?\d{4})(?:\D|$)").expect("phone regex")
    })
}

fn email_regex() -> &'static Regex {
    static EMAIL: OnceLock<Regex> = OnceLock::new();
    EMAIL.get_or_init(|| {
        Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").expect("email regex")
    })
}

/// GPA 数字分数：`3.86`、`3.9/4.0`（独立出现，非日期/年份）。
fn gpa_score_regex() -> &'static Regex {
    static GPA: OnceLock<Regex> = OnceLock::new();
    GPA.get_or_init(|| {
        Regex::new(r"(?:(?:GPA|绩点)[:：\s]*)?[0-4]\.\d{1,2}(?:\s*/\s*[0-4](?:\.\d+)?)?").expect("gpa regex")
    })
}

/// 城市标签值：`城市：上海`、`所在地:北京`、`现居：杭州`。
fn labeled_city(line: &str) -> Option<String> {
    static CITY: OnceLock<Regex> = OnceLock::new();
    let regex = CITY.get_or_init(|| {
        Regex::new(r"(?:城市|所在地|现居地|现居|居住地)[:：\s]*([^\s，,；;、]{1,20})")
            .expect("city label regex")
    });
    regex
        .captures(line)
        .map(|caps| caps[1].trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// 截断到 `max` 字符（按字符数），不切坏 UTF-8。
fn cut(text: &str, max: usize) -> String {
    text.trim().chars().take(max).collect()
}

fn clean_line(line: &str) -> &str {
    line.trim().trim_start_matches(['•', '·', '-', '—', '*', '·'])
}

/// 从联系区（首个段头之前的所有行）提取标量字段。
fn parse_contact_zone(lines: &[&str]) -> ResumeContent {
    let mut content = ResumeContent::default();
    let joined = lines.join("\n");

    if let Some(m) = email_regex().find(&joined) {
        content.email = cut(m.as_str(), MAX_FIELD_CHARS);
    }
    if let Some(caps) = phone_regex().captures(&joined) {
        if let Some(m) = caps.get(1) {
            content.phone = cut(&m.as_str().replace(['-', ' '], ""), MAX_FIELD_CHARS);
        }
    }
    // 标签字段：`城市：xx`，兼容一行混排多个标签（`邮箱：x 电话：y 现居：z`）。
    for line in lines {
        if let Some(value) = labeled_city(line) {
            content.city = cut(&value, MAX_FIELD_CHARS);
        }
    }
    // 姓名：首行、短、无数字/邮箱/分隔符。
    for line in lines {
        let candidate = clean_line(line);
        if candidate.is_empty() {
            continue;
        }
        let has_bad = candidate.chars().any(|ch| {
            ch.is_ascii_digit() || ch == '@' || ch == '|' || ch == '/' || ch == '：' || ch == ':'
        });
        if !has_bad && candidate.chars().count() <= 15 && detect_section(candidate).is_none() {
            content.name = cut(candidate, MAX_FIELD_CHARS);
        }
        break;
    }
    content
}

/// 经历条目：日期区间 + 条目头文字 + 描述行。
///
/// `headline` 已剥离日期文本；两行式布局（标题行与日期行分离）时
/// 由切分器合并为一段文字。
#[derive(Debug, Clone)]
struct Entry {
    start: String,
    end: String,
    headline: String,
    description: Vec<String>,
}

/// 切条目：段内以"含日期区间的行"为新条目锚点。
///
/// 支持三种常见布局：
/// 1. 单行式：`公司 职位 2023.06-2023.09`（锚行自带文字）；
/// 2. 日期独行：标题行在前、日期行单独一行（悬挂行回看合并）；
/// 3. 多行式：学校/公司/职位分散成多行后接日期行（如教育经历三行式）。
/// 锚点前保留最近 3 行悬挂行与锚行文字拼接；仍未被合并的散行（段头下的
/// 说明）丢弃。
fn split_entries(body_lines: &[&str]) -> Vec<Entry> {
    const MAX_PENDING_LINES: usize = 3;
    let mut entries: Vec<Entry> = Vec::new();
    let mut pending: Vec<String> = Vec::new();
    for line in body_lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match extract_date_range(trimmed) {
            Some((start, end, ds, de)) => {
                let rest = strip_span(trimmed, ds, de).trim().to_owned();
                let headline = if pending.is_empty() {
                    rest
                } else if rest.chars().count() >= 2 {
                    let mut parts = pending.clone();
                    parts.push(rest);
                    parts.join(" ")
                } else {
                    pending.join(" ")
                };
                pending.clear();
                entries.push(Entry {
                    start,
                    end,
                    headline,
                    description: Vec::new(),
                });
            }
            None => match entries.last_mut() {
                Some(entry) => entry.description.push(trimmed.to_owned()),
                None => {
                    if pending.len() >= MAX_PENDING_LINES {
                        pending.remove(0);
                    }
                    pending.push(trimmed.to_owned());
                }
            },
        }
    }
    entries
}

/// 去掉文本区间 `[start, end)`（字节偏移，regex 语义），拼接剩余部分。
fn strip_span(text: &str, start: usize, end: usize) -> String {
    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..start]);
    out.push_str(&text[end..]);
    out
}

/// 按常见分隔符切条目头文字（不含日期）。
fn split_headline(headline: &str) -> Vec<String> {
    headline
        .split(['|', '｜', '/', '·', '，', ',', '、', ';', '；', '@', ' ', '\u{3000}', '(', '（', ')', '）'])
        .map(str::trim)
        .filter(|chunk| !chunk.is_empty())
        .map(str::to_owned)
        .collect()
}

const COMPANY_KEYWORDS: &[&str] = &[
    "公司", "有限", "股份", "集团", "银行", "科技", "事业部", "厂", "工作室", "研究院",
    "研究所", "事务所", "中心", "网络", "传媒", "证券", "基金", "实验室", "医院", "商行",
    "inc", "ltd", "corp", "llc", "group",
];
const ROLE_KEYWORDS: &[&str] = &[
    "经理", "工程师", "实习", "管培生", "助理", "专员", "主管", "总监", "开发", "设计",
    "运营", "产品", "分析", "顾问", "教师", "讲师", "编辑", "记者", "策划", "翻译", "研究",
    "助教", "架构", "运维", "测试", "运营", "市场", "销售", "人事", "财务", "会计", "律师",
    "intern", "engineer", "manager", "lead", "analyst", "designer", "developer", "founder",
    "director", "architect", "consultant", "specialist", "officer", "assistant", "president",
    "researcher", "scientist",
];
const SCHOOL_KEYWORDS: &[&str] = &[
    "大学", "学院", "学校", "中学", "附属", "university", "college", "institute", "school",
];
const DEGREE_KEYWORDS: &[&str] = &[
    "博士", "硕士", "研究生", "本科", "学士", "大专", "专科", "在读", "应届", "双学位",
    "辅修", "mba", "phd", "ph.d", "doctor", "master", "bachelor", "associate", "b.s.", "m.s.",
    "b.a.", "m.a.",
];

/// 特征评分选块：任一正则词命中 +3，任一负则词命中 -3；纯数字块 -4，
/// 超长块（>30 字符）-2。返回得分 > 0 的最高分块；并列取先出现者。
fn pick_by_score<'a>(chunks: &'a [String], positive: &[&str], negative: &[&str]) -> Option<&'a String> {
    let mut best: Option<(i32, &'a String)> = None;
    for chunk in chunks {
        let lower = chunk.to_ascii_lowercase();
        let mut score = 0;
        if positive.iter().any(|key| lower.contains(key)) {
            score += 3;
        }
        if negative.iter().any(|key| lower.contains(key)) {
            score -= 3;
        }
        if !chunk.is_empty() && chunk.chars().all(|ch| ch.is_ascii_digit()) {
            score -= 4;
        }
        if chunk.chars().count() > 30 {
            score -= 2;
        }
        if score > 0 && best.as_ref().is_none_or(|(top, _)| score > *top) {
            best = Some((score, chunk));
        }
    }
    best.map(|(_, chunk)| chunk)
}

fn parse_education_entry(entry: &Entry) -> Option<ResumeEducation> {
    let chunks = split_headline(&entry.headline);
    let lower_chunks: Vec<String> = chunks.iter().map(|c| c.to_ascii_lowercase()).collect();

    let mut degree = chunks
        .iter()
        .zip(&lower_chunks)
        .find_map(|(chunk, lower)| {
            DEGREE_KEYWORDS
                .iter()
                .find(|key| lower.contains(**key))
                .map(|_| cut(chunk, MAX_ITEM_CHARS))
        })
        .unwrap_or_default();
    let mut school = pick_by_score(&chunks, SCHOOL_KEYWORDS, ROLE_KEYWORDS)
        .or_else(|| pick_by_score(&chunks, SCHOOL_KEYWORDS, DEGREE_KEYWORDS))
        .map(|chunk| cut(chunk, MAX_ITEM_CHARS))
        .unwrap_or_default();
    // 专业：去掉学校与学历块后的下一个块；否则看描述行。
    let major = chunks
        .iter()
        .filter(|chunk| {
            let lower = chunk.to_ascii_lowercase();
            !SCHOOL_KEYWORDS.iter().any(|key| lower.contains(*key))
                && !DEGREE_KEYWORDS.iter().any(|key| lower.contains(*key))
                && *chunk != &school
        })
        .next()
        .map(|chunk| cut(chunk, MAX_ITEM_CHARS))
        .unwrap_or_default();

    // 两行式布局兜底：学校/学历缺失时从描述行补抓（如标题行只有学校、
    // "本科 · 计算机科学与技术"在下一行）。
    if school.is_empty() {
        school = entry
            .description
            .iter()
            .find(|line| {
                let lower = line.to_ascii_lowercase();
                SCHOOL_KEYWORDS.iter().any(|key| lower.contains(*key))
            })
            .map(|line| cut(line, MAX_ITEM_CHARS))
            .unwrap_or_default();
    }
    if degree.is_empty() {
        degree = entry
            .description
            .iter()
            .find(|line| {
                let lower = line.to_ascii_lowercase();
                DEGREE_KEYWORDS.iter().any(|key| lower.contains(*key))
            })
            .map(|line| cut(line, MAX_ITEM_CHARS))
            .unwrap_or_default();
    }
    // 排名/GPA：描述行命中关键词，或出现 `x.xx` 数字分数。
    let ranking = entry
        .description
        .iter()
        .find(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("gpa")
                || lower.contains("绩点")
                || lower.contains("排名")
                || lower.contains("前")
                || gpa_score_regex().is_match(line)        })
        .map(|line| cut(line, MAX_ITEM_CHARS))
        .unwrap_or_default();

    if school.is_empty() && major.is_empty() && degree.is_empty() {
        return None;
    }
    Some(ResumeEducation {
        school,
        degree,
        major,
        start_date: cut(&entry.start, MAX_ITEM_CHARS),
        end_date: cut(&entry.end, MAX_ITEM_CHARS),
        ranking,
    })
}

fn parse_experience_entry(entry: &Entry, internship: bool) -> Option<ResumeExperience> {
    let chunks = split_headline(&entry.headline);
    let company = pick_by_score(&chunks, COMPANY_KEYWORDS, ROLE_KEYWORDS)
        .map(|chunk| cut(chunk, MAX_ITEM_CHARS))
        .unwrap_or_else(|| chunks.first().map(|c| cut(c, MAX_ITEM_CHARS)).unwrap_or_default());
    let role = pick_by_score(&chunks, ROLE_KEYWORDS, COMPANY_KEYWORDS)
        .map(|chunk| cut(chunk, MAX_ITEM_CHARS))
        .unwrap_or_else(|| chunks.get(1).map(|c| cut(c, MAX_ITEM_CHARS)).unwrap_or_default());
    let kind = if internship || entry.headline.contains("实习") {
        "实习"
    } else {
        "工作"
    };
    if company.is_empty() && role.is_empty() {
        return None;
    }
    Some(ResumeExperience {
        company,
        role,
        kind: kind.to_owned(),
        start_date: cut(&entry.start, MAX_ITEM_CHARS),
        end_date: cut(&entry.end, MAX_ITEM_CHARS),
        description: cut(&entry.description.join("\n"), MAX_ITEM_DESCRIPTION_CHARS),
    })
}

fn parse_named_entry(entry: &Entry) -> Option<(String, String, String, String)> {
    let chunks = split_headline(&entry.headline);
    let name = chunks.first().map(|chunk| cut(chunk, MAX_ITEM_CHARS))?;
    let role = chunks
        .get(1)
        .map(|chunk| cut(chunk, MAX_ITEM_CHARS))
        .unwrap_or_default();
    Some((
        name,
        role,
        cut(&entry.start, MAX_ITEM_CHARS),
        cut(&entry.end, MAX_ITEM_CHARS),
    ))
}

/// 主入口：纯文本 → 结构化草稿。
/// 所有写入字段先过字符上限截断；解析不出任何内容时退回 summary 兜底。
pub fn structurize(text: &str) -> ResumeContent {
    let text = text.trim();
    if text.is_empty() {
        return ResumeContent::default();
    }
    let lines: Vec<&str> = text.lines().collect();

    // 1. 按段头切分：联系区 + 各段正文。
    let mut contact_lines: Vec<&str> = Vec::new();
    let mut sections: Vec<(Section, Vec<&str>)> = Vec::new();
    for line in &lines {
        match detect_section(line) {
            Some(section) => sections.push((section, Vec::new())),
            None => match sections.last_mut() {
                Some((_, body)) => body.push(line),
                None => contact_lines.push(line),
            },
        }
    }

    let mut content = parse_contact_zone(&contact_lines);

    // 2. 逐段结构化。
    for (section, body) in &sections {
        match section {
            Section::Summary => {
                let summary: String = body
                    .iter()
                    .map(|line| line.trim())
                    .filter(|line| !line.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n");
                if !summary.is_empty() {
                    content.summary = cut(&summary, MAX_SUMMARY_CHARS);
                }
            }
            Section::Education => {
                for entry in split_entries(body) {
                    if let Some(item) = parse_education_entry(&entry) {
                        content.educations.push(item);
                    }
                }
            }
            Section::Experience { internship } => {
                for entry in split_entries(body) {
                    if let Some(item) = parse_experience_entry(&entry, *internship) {
                        content.experiences.push(item);
                    }
                }
            }
            Section::Projects => {
                for entry in split_entries(body) {
                    if let Some((name, role, start, end)) = parse_named_entry(&entry) {
                        content.projects.push(ResumeProject {
                            name,
                            role,
                            start_date: start,
                            end_date: end,
                            description: cut(&entry.description.join("\n"), MAX_ITEM_DESCRIPTION_CHARS),
                        });
                    }
                }
            }
            Section::Practices => {
                for entry in split_entries(body) {
                    if let Some((name, role, start, end)) = parse_named_entry(&entry) {
                        content.practices.push(ResumePractice {
                            name,
                            role,
                            start_date: start,
                            end_date: end,
                            description: cut(&entry.description.join("\n"), MAX_ITEM_DESCRIPTION_CHARS),
                        });
                    }
                }
            }
            Section::Skills => {
                for line in body {
                    let line = clean_line(line);
                    if line.is_empty() {
                        continue;
                    }
                    for token in line.split(['、', '，', ',', ';', '；', '/', '|', '·']) {
                        let name = token.trim();
                        if !name.is_empty() {
                            content.skills.push(ResumeSkill {
                                name: cut(name, MAX_ITEM_CHARS),
                                level: String::new(),
                            });
                        }
                    }
                }
            }
            Section::Certificates => {
                for line in body {
                    let line = clean_line(line);
                    if line.is_empty() {
                        continue;
                    }
                    let (name, date) = split_trailing_date(line);
                    content.certificates.push(ResumeCertificate {
                        name: cut(&name, MAX_ITEM_CHARS),
                        date: cut(&date, MAX_ITEM_CHARS),
                    });
                }
            }
            Section::Honors => {
                for line in body {
                    let line = clean_line(line);
                    if line.is_empty() {
                        continue;
                    }
                    let (name, date) = split_trailing_date(line);
                    let level = ["国家级", "省级", "市级", "校级", "院级"]
                        .into_iter()
                        .find(|key| name.contains(*key))
                        .unwrap_or("");
                    content.honors.push(ResumeHonor {
                        name: cut(&name, MAX_ITEM_CHARS),
                        level: level.to_owned(),
                        date: cut(&date, MAX_ITEM_CHARS),
                    });
                }
            }
        }
    }

    // 3. 条目数上限。
    content.educations.truncate(super::content::MAX_ITEMS);
    content.experiences.truncate(super::content::MAX_ITEMS);
    content.projects.truncate(super::content::MAX_ITEMS);
    content.practices.truncate(super::content::MAX_ITEMS);
    content.skills.truncate(super::content::MAX_ITEMS);
    content.certificates.truncate(super::content::MAX_ITEMS);
    content.honors.truncate(super::content::MAX_ITEMS);

    // 4. 兜底：没有任何可识别结构（段头/联系方式）时，弱猜测的姓名不可信，
    // 整段原文退回 summary（保持旧行为，人工再整理）。
    let strong = !sections.is_empty()
        || !content.email.is_empty()
        || !content.phone.is_empty()
        || !content.city.is_empty();
    if !strong {
        let mut fallback = ResumeContent::default();
        fallback.summary = cut(text, MAX_SUMMARY_CHARS);
        return fallback;
    }
    content
}

/// 行尾日期拆分：`大学英语四级 2021.06` → (名称, 日期)。
fn split_trailing_date(line: &str) -> (String, String) {
    let mut name = line.trim().to_owned();
    let mut date = String::new();
    let positions: Vec<(usize, usize)> = date_token_positions(line).collect();
    if let Some((start, end)) = positions.last() {
        // 只有当日期在行尾附近（其后只有 ≤2 个非空字符）才视为行尾日期。
        let tail = line[*end..].trim();
        if tail.chars().count() <= 2 {
            date = normalize_date(&line[*start..*end]);
            name = line[..*start].trim().trim_end_matches(['，', ',', '、', '-']).to_owned();
        }
    }
    (name, date)
}

/// 回填合并：只填空字段；列表追加去重；summary 保留已有值。
pub fn merge_parsed(base: &ResumeContent, parsed: &ResumeContent) -> ResumeContent {
    let mut merged = base.clone();
    let fill = |target: &mut String, source: &str| {
        if target.trim().is_empty() {
            *target = source.to_owned();
        }
    };
    fill(&mut merged.name, &parsed.name);
    fill(&mut merged.phone, &parsed.phone);
    fill(&mut merged.email, &parsed.email);
    fill(&mut merged.city, &parsed.city);
    fill(&mut merged.target_role, &parsed.target_role);
    fill(&mut merged.target_city, &parsed.target_city);
    fill(&mut merged.job_type, &parsed.job_type);
    fill(&mut merged.available_date, &parsed.available_date);
    if merged.summary.trim().is_empty() {
        merged.summary = parsed.summary.clone();
    }

    for item in &parsed.educations {
        push_unique(&mut merged.educations, item.clone());
    }
    for item in &parsed.experiences {
        push_unique(&mut merged.experiences, item.clone());
    }
    for item in &parsed.projects {
        push_unique(&mut merged.projects, item.clone());
    }
    for item in &parsed.practices {
        push_unique(&mut merged.practices, item.clone());
    }
    for item in &parsed.skills {
        push_unique(&mut merged.skills, item.clone());
    }
    for item in &parsed.certificates {
        push_unique(&mut merged.certificates, item.clone());
    }
    for item in &parsed.honors {
        push_unique(&mut merged.honors, item.clone());
    }
    merged
}

/// 追加去重并保持条目上限。
fn push_unique<T: PartialEq + Clone>(list: &mut Vec<T>, item: T) {
    if !list.contains(&item) && list.len() < super::content::MAX_ITEMS {
        list.push(item);
    }
}
