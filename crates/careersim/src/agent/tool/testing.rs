//! 测试用的工具替身。

use async_trait::async_trait;

use super::Tool;
use super::ToolContext;
use super::ToolError;
use super::ToolOutput;
use super::ToolSpec;

pub struct FakeTool {
    spec: ToolSpec,
    output: String,
}

impl FakeTool {
    pub fn new(name: &str, output: &str) -> Self {
        Self {
            spec: ToolSpec {
                name: name.to_owned(),
                description: format!("fake tool {name}"),
                parameters: serde_json::json!({"type": "object"}),
                strict: false,
            },
            output: output.to_owned(),
        }
    }
}

#[async_trait]
impl Tool for FakeTool {
    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    async fn call(
        &self,
        _arguments: serde_json::Value,
        _ctx: &ToolContext<'_>,
    ) -> Result<ToolOutput, ToolError> {
        Ok(ToolOutput {
            content: self.output.clone(),
        })
    }
}
