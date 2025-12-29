use rmcp::{
    model::{CallToolRequestParam, Tool},
    service::RunningService,
    transport::{ConfigureCommandExt, TokioChildProcess},
    RoleClient, ServiceExt,
};
use std::sync::Arc;
use thiserror::Error;
use tokio::process::Command;
use tokio::sync::RwLock;

#[derive(Error, Debug)]
pub enum McpError {
    #[error("Failed to spawn MCP server: {0}")]
    Spawn(String),
    #[error("Failed to list tools: {0}")]
    ListTools(String),
    #[error("Failed to call tool: {0}")]
    CallTool(String),
    #[error("MCP client not initialized")]
    NotInitialized,
}

pub struct McpClient {
    service: Option<RunningService<RoleClient, ()>>,
    tools: Vec<Tool>,
}

impl McpClient {
    pub fn new() -> Self {
        Self {
            service: None,
            tools: Vec::new(),
        }
    }

    pub async fn connect(&mut self, command: &str, args: &[&str]) -> Result<(), McpError> {
        let args_owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();

        let service = ()
            .serve(
                TokioChildProcess::new(Command::new(command).configure(|cmd| {
                    for arg in &args_owned {
                        cmd.arg(arg);
                    }
                }))
                .map_err(|e| McpError::Spawn(e.to_string()))?,
            )
            .await
            .map_err(|e| McpError::Spawn(e.to_string()))?;

        // list tools from server
        let tools_result = service
            .list_tools(Default::default())
            .await
            .map_err(|e| McpError::ListTools(e.to_string()))?;

        self.tools = tools_result.tools;
        self.service = Some(service);

        println!(
            "[mcp] Connected, found {} tools: {:?}",
            self.tools.len(),
            self.tools.iter().map(|t| &t.name).collect::<Vec<_>>()
        );

        Ok(())
    }

    pub fn get_tools_for_claude(&self) -> Vec<serde_json::Value> {
        self.tools
            .iter()
            .map(|tool| {
                serde_json::json!({
                    "name": tool.name,
                    "description": tool.description.clone().unwrap_or_default(),
                    "input_schema": tool.input_schema
                })
            })
            .collect()
    }

    pub fn get_tool_names(&self) -> Vec<String> {
        self.tools.iter().map(|t| t.name.to_string()).collect()
    }

    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.iter().any(|t| t.name == name)
    }

    pub async fn call_tool(
        &self,
        name: &str,
        arguments: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<String, McpError> {
        let service = self.service.as_ref().ok_or(McpError::NotInitialized)?;

        let tool_name = name.to_string();
        let result = service
            .call_tool(CallToolRequestParam {
                name: tool_name.into(),
                arguments,
            })
            .await
            .map_err(|e| McpError::CallTool(e.to_string()))?;

        // extract text content from result
        let output = result
            .content
            .iter()
            .filter_map(|c| {
                if let rmcp::model::RawContent::Text(text) = &**c {
                    Some(text.text.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok(output)
    }

    pub async fn disconnect(&mut self) {
        if let Some(service) = self.service.take() {
            let _ = service.cancel().await;
        }
        self.tools.clear();
    }

    pub fn is_connected(&self) -> bool {
        self.service.is_some()
    }
}

// thread-safe wrapper
pub type SharedMcpClient = Arc<RwLock<McpClient>>;

pub fn create_shared_client() -> SharedMcpClient {
    Arc::new(RwLock::new(McpClient::new()))
}
