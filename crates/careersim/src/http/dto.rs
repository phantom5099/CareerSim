//! 通用分页 DTO（plan/00 §6.1）：`?limit=20&cursor=...`，limit 上限 100。

use serde::{Deserialize, Serialize};

/// 列表查询参数；`limit` 超限或非法时按默认值收敛，避免整页错误。
#[derive(Debug, Deserialize)]
pub struct PageQuery {
    #[serde(default = "default_limit")]
    pub limit: u32,
    pub cursor: Option<String>,
}

impl Default for PageQuery {
    fn default() -> Self {
        Self {
            limit: default_limit(),
            cursor: None,
        }
    }
}

impl PageQuery {
    /// 收敛后的 limit，供 SQL LIMIT 使用。
    pub fn effective_limit(&self) -> u32 {
        if self.limit == 0 || self.limit > PAGE_LIMIT_MAX {
            PAGE_LIMIT_DEFAULT
        } else {
            self.limit
        }
    }
}

const PAGE_LIMIT_DEFAULT: u32 = 20;
const PAGE_LIMIT_MAX: u32 = 100;

fn default_limit() -> u32 {
    PAGE_LIMIT_DEFAULT
}

/// 分页响应；`nextCursor` 为空表示没有更多数据。
#[derive(Debug, Serialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    #[serde(rename = "nextCursor", skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

impl<T> Page<T> {
    pub fn of(items: Vec<T>, next_cursor: Option<String>) -> Self {
        Self { items, next_cursor }
    }
}
