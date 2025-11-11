use crate::browser::BrowserSession;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars,
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map as JsonMap, Value as JsonValue};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};

/// Parameters for updating an account
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct UpdateAccountParams {
    /// The ID of the account to update
    pub account_id: String,
    /// The new data for the account as a JSON object
    pub data: JsonMap<String, JsonValue>,
    /// Allow assignment of new properties to account objects
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force: Option<bool>,
}

/// Parameters for restoring Current Finances
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct RestoreCurrentFinancesParams {
    /// The new Current Finances state as a JSON object
    pub new_state: JsonMap<String, JsonValue>,
}

/// Parameters for restoring Plans
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct RestorePlansParams {
    /// The new plans data as a JSON object
    pub new_plans: JsonMap<String, JsonValue>,
}

/// Parameters for restoring Progress
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct RestoreProgressParams {
    /// The new progress data as a JSON object
    pub new_progress: JsonMap<String, JsonValue>,
}

/// Parameters for restoring Settings
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct RestoreSettingsParams {
    /// The new settings data as a JSON object
    pub new_settings: JsonMap<String, JsonValue>,
}

/// Main MCP server for ProjectionLab integration
#[derive(Clone)]
pub struct ProjectionLabServer {
    browser: Arc<Mutex<Option<BrowserSession>>>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl ProjectionLabServer {
    pub fn new() -> Self {
        Self {
            browser: Arc::new(Mutex::new(None)),
            tool_router: Self::tool_router(),
        }
    }

    /// Helper to get browser session or return error
    async fn get_browser(&self) -> Result<tokio::sync::MutexGuard<'_, Option<BrowserSession>>, McpError> {
        Ok(self.browser.lock().await)
    }

    #[tool(description = "Update an account in Current Finances with new data")]
    async fn update_account(
        &self,
        Parameters(params): Parameters<UpdateAccountParams>,
    ) -> Result<CallToolResult, McpError> {
        let browser_guard = self.get_browser().await?;
        let browser = browser_guard
            .as_ref()
            .ok_or_else(|| McpError::internal_error("Browser session not initialized", None))?;

        let mut args = vec![
            json!(params.account_id),
            json!(params.data),
        ];

        if let Some(f) = params.force {
            args.push(json!({ "force": f }));
        }

        let result = browser
            .call_plugin_api("updateAccount", args)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Account {} updated successfully: {}",
            params.account_id,
            serde_json::to_string_pretty(&result).unwrap_or_default()
        ))]))
    }

    #[tool(description = "Export all financial data from ProjectionLab")]
    async fn export_data(&self) -> Result<CallToolResult, McpError> {
        let browser_guard = self.get_browser().await?;
        let browser = browser_guard
            .as_ref()
            .ok_or_else(|| McpError::internal_error("Browser session not initialized", None))?;

        let result = browser
            .call_plugin_api("exportData", vec![])
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default()
        )]))
    }

    #[tool(description = "Replace the Current Finances state with new data")]
    async fn restore_current_finances(
        &self,
        Parameters(params): Parameters<RestoreCurrentFinancesParams>,
    ) -> Result<CallToolResult, McpError> {
        let browser_guard = self.get_browser().await?;
        let browser = browser_guard
            .as_ref()
            .ok_or_else(|| McpError::internal_error("Browser session not initialized", None))?;

        let result = browser
            .call_plugin_api("restoreCurrentFinances", vec![json!(params.new_state)])
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Current Finances restored successfully: {}",
            serde_json::to_string_pretty(&result).unwrap_or_default()
        ))]))
    }

    #[tool(description = "Replace all Plans with a new set of plans")]
    async fn restore_plans(
        &self,
        Parameters(params): Parameters<RestorePlansParams>,
    ) -> Result<CallToolResult, McpError> {
        let browser_guard = self.get_browser().await?;
        let browser = browser_guard
            .as_ref()
            .ok_or_else(|| McpError::internal_error("Browser session not initialized", None))?;

        let result = browser
            .call_plugin_api("restorePlans", vec![json!(params.new_plans)])
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Plans restored successfully: {}",
            serde_json::to_string_pretty(&result).unwrap_or_default()
        ))]))
    }

    #[tool(description = "Replace the Progress state with new data")]
    async fn restore_progress(
        &self,
        Parameters(params): Parameters<RestoreProgressParams>,
    ) -> Result<CallToolResult, McpError> {
        let browser_guard = self.get_browser().await?;
        let browser = browser_guard
            .as_ref()
            .ok_or_else(|| McpError::internal_error("Browser session not initialized", None))?;

        let result = browser
            .call_plugin_api("restoreProgress", vec![json!(params.new_progress)])
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Progress restored successfully: {}",
            serde_json::to_string_pretty(&result).unwrap_or_default()
        ))]))
    }

    #[tool(description = "Replace Settings state with new data")]
    async fn restore_settings(
        &self,
        Parameters(params): Parameters<RestoreSettingsParams>,
    ) -> Result<CallToolResult, McpError> {
        let browser_guard = self.get_browser().await?;
        let browser = browser_guard
            .as_ref()
            .ok_or_else(|| McpError::internal_error("Browser session not initialized", None))?;

        let result = browser
            .call_plugin_api("restoreSettings", vec![json!(params.new_settings)])
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Settings restored successfully: {}",
            serde_json::to_string_pretty(&result).unwrap_or_default()
        ))]))
    }

    #[tool(description = "Validate that the cached API key is still valid")]
    async fn validate_api_key(&self) -> Result<CallToolResult, McpError> {
        let browser_guard = self.get_browser().await?;
        let browser = browser_guard
            .as_ref()
            .ok_or_else(|| McpError::internal_error("Browser session not initialized", None))?;

        let result = browser
            .call_plugin_api("validateApiKey", vec![])
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "API key validation result: {}",
            serde_json::to_string_pretty(&result).unwrap_or_default()
        ))]))
    }
}

#[tool_handler]
impl ServerHandler for ProjectionLabServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation {
                name: "projectionlab-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                title: Some("ProjectionLab MCP Server".to_string()),
                website_url: Some("https://github.com/yourusername/projectionlab-mcp".to_string()),
                icons: None,
            },
            instructions: Some(
                "ProjectionLab MCP Server - Interact with ProjectionLab personal finance software.\n\n\
                Available tools:\n\
                - update_account: Update account balances and data\n\
                - export_data: Export all financial data\n\
                - restore_current_finances: Replace Current Finances state\n\
                - restore_plans: Replace all Plans\n\
                - restore_progress: Replace Progress state\n\
                - restore_settings: Replace Settings\n\
                - validate_api_key: Validate API key\n\n\
                The server will launch Firefox on first connection and wait for you to log in to ProjectionLab."
                    .to_string(),
            ),
        }
    }

    async fn initialize(
        &self,
        _request: InitializeRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        info!("Initializing ProjectionLab MCP Server...");

        // Initialize browser session
        let mut browser_guard = self.browser.lock().await;

        if browser_guard.is_none() {
            info!("Creating new browser session...");

            match BrowserSession::new().await {
                Ok(session) => {
                    *browser_guard = Some(session);
                    info!("Browser session initialized successfully!");
                }
                Err(e) => {
                    error!("Failed to initialize browser session: {}", e);
                    return Err(McpError::internal_error(
                        format!("Failed to initialize browser: {}", e),
                        None,
                    ));
                }
            }
        }

        Ok(self.get_info())
    }
}
