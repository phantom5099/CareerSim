//! 公司行业标签等只读元数据。

use axum::Json;
use serde::Serialize;

/// 行业标签目录。当前为内置清单，后续可切换到数据库或配置中心。
const INDUSTRIES: &[&str] = &[
    "人工智能",
    "设计服务",
    "云计算",
    "文化传媒",
    "出行服务",
    "计算机软件",
    "智能硬件",
    "数据服务",
    "游戏",
    "电子商务",
    "金融科技",
    "教育培训",
    "医疗健康",
];

#[derive(Serialize)]
pub struct IndustriesResponse {
    pub industries: Vec<&'static str>,
}

pub async fn industries() -> Json<IndustriesResponse> {
    Json(IndustriesResponse {
        industries: INDUSTRIES.to_vec(),
    })
}
