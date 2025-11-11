use anyhow::{Context, Result};
use serde_json::Value;
use std::time::Duration;
use thirtyfour::prelude::*;
use tokio::process::{Child, Command};
use tracing::{info, warn};

const PROJECTIONLAB_URL: &str = "https://app.projectionlab.com";
const PLUGINS_SETTINGS_URL: &str = "https://app.projectionlab.com/settings/plugins";
const LOGIN_WAIT_TIMEOUT_SECS: u64 = 300; // 5 minutes for user to log in
const GECKODRIVER_URL: &str = "http://localhost:4444";
const GECKODRIVER_PORT: u16 = 4444;

/// Manages the Firefox browser session for interacting with ProjectionLab
pub struct BrowserSession {
    driver: WebDriver,
    api_key: Option<String>,
    /// Optional GeckoDriver child process (if we started it)
    geckodriver_process: Option<Child>,
}

impl BrowserSession {
    /// Creates a new browser session by launching Firefox and waiting for user login
    pub async fn new() -> Result<Self> {
        // Start GeckoDriver if it's not already running
        let geckodriver_process = Self::ensure_geckodriver_running().await?;

        info!("Launching Firefox browser...");

        // Configure Firefox capabilities
        let caps = DesiredCapabilities::firefox();

        // Start in headed (visible) mode - Firefox caps default to non-headless
        // To enable headless, you would call: caps.set_headless()?;

        // Connect to GeckoDriver
        let driver = WebDriver::new(GECKODRIVER_URL, caps)
            .await
            .context("Failed to connect to GeckoDriver")?;

        info!("Firefox launched successfully");

        let mut session = Self {
            driver,
            api_key: None,
            geckodriver_process,
        };

        // Navigate to ProjectionLab and wait for user to log in
        session.initialize_session().await?;

        Ok(session)
    }

    /// Ensures GeckoDriver is running, starting it if necessary
    /// Returns Some(Child) if we started it, None if it was already running
    async fn ensure_geckodriver_running() -> Result<Option<Child>> {
        // Check if GeckoDriver is already running by attempting to connect
        if Self::is_geckodriver_running().await {
            info!("GeckoDriver is already running on port {}", GECKODRIVER_PORT);
            return Ok(None);
        }

        info!("Starting GeckoDriver on port {}...", GECKODRIVER_PORT);

        // Start GeckoDriver as a child process
        let child = Command::new("geckodriver")
            .arg("--port")
            .arg(GECKODRIVER_PORT.to_string())
            .stdout(std::process::Stdio::null()) // Suppress stdout
            .stderr(std::process::Stdio::null()) // Suppress stderr
            .spawn()
            .context("Failed to start GeckoDriver. Is it installed?")?;

        info!("GeckoDriver started successfully (PID: {:?})", child.id());

        // Give GeckoDriver a moment to start up
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Verify it's running
        for i in 0..10 {
            if Self::is_geckodriver_running().await {
                info!("GeckoDriver is ready");
                return Ok(Some(child));
            }
            if i < 9 {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }

        anyhow::bail!("GeckoDriver started but didn't become ready in time")
    }

    /// Checks if GeckoDriver is running by attempting a simple TCP connection
    async fn is_geckodriver_running() -> bool {
        tokio::net::TcpStream::connect(("127.0.0.1", GECKODRIVER_PORT))
            .await
            .is_ok()
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

    /// Waits for the user to log in by periodically checking the URL without navigating away
    async fn wait_for_login(&self) -> Result<()> {
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(LOGIN_WAIT_TIMEOUT_SECS);

        loop {
            // Check if login timeout has been reached
            if start.elapsed() > timeout {
                anyhow::bail!("Login timeout: User did not log in within {} seconds", LOGIN_WAIT_TIMEOUT_SECS);
            }

            // Check current URL without navigating
            match self.driver.current_url().await {
                Ok(url) => {
                    let url_str = url.as_str();

                    // Check if we're on a logged-in page (not on login/auth pages)
                    // Look for indicators that we're logged in:
                    // - Not on /login or /auth pages
                    // - On app.projectionlab.com (not redirected elsewhere)
                    if url_str.contains("app.projectionlab.com")
                        && !url_str.contains("/login")
                        && !url_str.contains("/auth")
                        && !url_str.contains("/signin") {

                        info!("Login detected - user is on: {}", url_str);
                        return Ok(());
                    }
                }
                Err(e) => {
                    warn!("Error checking current URL: {}", e);
                }
            }

            // Wait before checking again (don't navigate, just wait)
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

}

impl Drop for BrowserSession {
    fn drop(&mut self) {
        // If we started GeckoDriver, kill it when BrowserSession is dropped
        if let Some(child) = &mut self.geckodriver_process {
            if let Some(pid) = child.id() {
                info!("BrowserSession dropped - stopping GeckoDriver (PID: {})...", pid);
                // We can't await in Drop, so start_kill() initiates termination without waiting
                if let Err(e) = child.start_kill() {
                    warn!("Error killing GeckoDriver: {}", e);
                } else {
                    info!("GeckoDriver termination initiated");
                }
            }
        }
    }
}
