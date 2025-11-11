use anyhow::{Context, Result};
use serde_json::Value;
use std::time::Duration;
use thirtyfour::prelude::*;
use tracing::{info, warn};

const PROJECTIONLAB_URL: &str = "https://app.projectionlab.com";
const PLUGINS_SETTINGS_URL: &str = "https://app.projectionlab.com/settings/plugins";
const LOGIN_WAIT_TIMEOUT_SECS: u64 = 300; // 5 minutes for user to log in

/// Manages the Firefox browser session for interacting with ProjectionLab
pub struct BrowserSession {
    driver: WebDriver,
    api_key: Option<String>,
}

impl BrowserSession {
    /// Creates a new browser session by launching Firefox and waiting for user login
    pub async fn new() -> Result<Self> {
        info!("Launching Firefox browser...");

        // Configure Firefox capabilities
        let caps = DesiredCapabilities::firefox();

        // Start in headed (visible) mode - Firefox caps default to non-headless
        // To enable headless, you would call: caps.set_headless()?;

        // Connect to GeckoDriver
        let driver = WebDriver::new("http://localhost:4444", caps)
            .await
            .context("Failed to connect to GeckoDriver. Is it running?")?;

        info!("Firefox launched successfully");

        let mut session = Self {
            driver,
            api_key: None,
        };

        // Navigate to ProjectionLab and wait for user to log in
        session.initialize_session().await?;

        Ok(session)
    }

    /// Initializes the session by navigating to ProjectionLab and retrieving the API key
    async fn initialize_session(&mut self) -> Result<()> {
        info!("Navigating to ProjectionLab...");
        self.driver.goto(PROJECTIONLAB_URL).await?;

        info!("Waiting for user to log in...");
        info!("Please log in to ProjectionLab in the browser window.");

        // Wait for user to log in by checking if we can access the plugins page
        self.wait_for_login().await?;

        info!("Login detected! Navigating to plugins settings...");

        // Navigate to plugins settings page
        self.driver.goto(PLUGINS_SETTINGS_URL).await?;

        // Give the page time to load
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Extract API key from the page
        self.api_key = Some(self.extract_api_key().await?);

        info!("API key retrieved successfully!");

        Ok(())
    }

    /// Waits for the user to log in by periodically checking if we can access authenticated pages
    async fn wait_for_login(&self) -> Result<()> {
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(LOGIN_WAIT_TIMEOUT_SECS);

        loop {
            // Check if login timeout has been reached
            if start.elapsed() > timeout {
                anyhow::bail!("Login timeout: User did not log in within {} seconds", LOGIN_WAIT_TIMEOUT_SECS);
            }

            // Try to navigate to settings page to check if logged in
            match self.driver.goto(PLUGINS_SETTINGS_URL).await {
                Ok(_) => {
                    // Check if we're actually on the settings page or redirected to login
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    let current_url = self.driver.current_url().await?;

                    if current_url.as_str().contains("/settings/plugins") {
                        // Successfully on settings page, user is logged in
                        return Ok(());
                    }
                }
                Err(e) => {
                    warn!("Error checking login status: {}", e);
                }
            }

            // Wait before trying again
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    /// Extracts the API key from the plugins settings page
    async fn extract_api_key(&self) -> Result<String> {
        // Try multiple selectors to find the API key
        // This will need to be adjusted based on the actual DOM structure
        let selectors = vec![
            "input[type='text'][readonly]", // Common pattern for API key inputs
            "input[name='apiKey']",
            "input[id='apiKey']",
            "code", // API keys are often displayed in <code> tags
        ];

        for selector in selectors {
            if let Ok(element) = self.driver.find(By::Css(selector)).await {
                if let Ok(value) = element.text().await {
                    if !value.is_empty() && value.len() > 10 {
                        return Ok(value);
                    }
                }

                // Try to get value attribute for inputs
                if let Ok(value) = element.attr("value").await {
                    if let Some(v) = value {
                        if !v.is_empty() && v.len() > 10 {
                            return Ok(v);
                        }
                    }
                }
            }
        }

        // If we can't find it with selectors, try executing JavaScript
        let script = r#"
            // Try to find API key in various ways
            const inputs = document.querySelectorAll('input[readonly], input[type="text"]');
            for (const input of inputs) {
                if (input.value && input.value.length > 10) {
                    return input.value;
                }
            }

            // Check code elements
            const codes = document.querySelectorAll('code');
            for (const code of codes) {
                if (code.textContent && code.textContent.length > 10) {
                    return code.textContent;
                }
            }

            return null;
        "#;

        let result = self.driver.execute(script, vec![]).await?;

        // Get the JSON value from the script result
        let value = result.json();
        if let Some(api_key_str) = value.as_str() {
            return Ok(api_key_str.to_string());
        }

        anyhow::bail!("Could not find API key on plugins settings page. Please ensure you're on the correct page.")
    }

    /// Returns the cached API key
    pub fn api_key(&self) -> Result<&str> {
        self.api_key
            .as_deref()
            .context("API key not initialized")
    }

    /// Executes a ProjectionLab Plugin API call via JavaScript
    pub async fn call_plugin_api(
        &self,
        method: &str,
        args: Vec<Value>,
    ) -> Result<Value> {
        let api_key = self.api_key()?;

        // Build the JavaScript call
        let args_with_key = {
            let mut all_args = args;
            // Add API key as last argument with {key: "..."} format
            all_args.push(serde_json::json!({ "key": api_key }));
            all_args
        };

        let args_json = serde_json::to_string(&args_with_key)?;

        let script = format!(
            r#"
            return await window.projectionlabPluginAPI.{}(...{});
            "#,
            method, args_json
        );

        info!("Executing plugin API call: {}", method);

        let result = self.driver.execute(&script, vec![]).await
            .context(format!("Failed to execute plugin API method: {}", method))?;

        // Convert ScriptRet to serde_json::Value by cloning the internal JSON
        Ok(result.json().clone())
    }

    /// Closes the browser session
    pub async fn close(self) -> Result<()> {
        info!("Closing browser session...");
        self.driver.quit().await?;
        Ok(())
    }
}
