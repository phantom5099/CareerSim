//! 用户资料。演示版只读，不含密码等凭据字段。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub username: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub school: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub major: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graduation_year: Option<u16>,
    pub target_roles: Vec<String>,
    pub target_cities: Vec<String>,
    pub avatar_text: String,
    pub avatar_color: String,
}
