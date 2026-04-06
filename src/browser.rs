use anyhow::{Context, Result};
use backoff::{backoff::Backoff, ExponentialBackoffBuilder};
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

/// Returns the path to the persistent Firefox profile directory.
/// Session cookies and login state survive between launches.
fn profile_dir() -> std::path::PathBuf {
    let dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("projectionlab-mcp")
        .join("firefox-profile");
    std::fs::create_dir_all(&dir).ok();
    dir
}

/// Manages the Firefox browser session for interacting with ProjectionLab
pub struct BrowserSession {
    driver: Option<WebDriver>,
    api_key: Option<String>,
    /// Optional GeckoDriver child process (if we started it)
    geckodriver_process: Option<Child>,
    /// Last path navigated to (for skipping redundant navigations)
    last_navigated_path: Option<String>,
}

impl BrowserSession {
    /// Creates a new browser session.
    ///
    /// Strategy:
    /// 1. Launch headless Firefox with persistent profile
    /// 2. Navigate to ProjectionLab — if session cookies are valid, we're logged in
    /// 3. If login is needed, quit headless, relaunch visible for manual login, then quit and relaunch headless
    pub async fn new() -> Result<Self> {
        let geckodriver_process = Self::ensure_geckodriver_running().await?;
        let profile = profile_dir();
        let profile_str = profile.to_str().context("Non-UTF8 profile path")?.to_string();

        info!("Using Firefox profile: {}", profile_str);

        // Step 1: Try headless first
        info!("Launching headless Firefox...");
        let driver = Self::launch_firefox(&profile_str, true).await?;

        let mut session = Self {
            driver: Some(driver),
            api_key: None,
            geckodriver_process,
            last_navigated_path: None,
        };

        // Step 2: Check if we're already logged in
        session.driver()?.goto(PROJECTIONLAB_URL).await?;
        tokio::time::sleep(Duration::from_secs(2)).await;

        let logged_in = session.check_logged_in().await;

        if !logged_in {
            // Step 3: Need manual login — switch to visible browser
            info!("Session expired, launching visible browser for login...");

            // Quit headless
            if let Some(driver) = session.driver.take() {
                let _ = driver.quit().await;
            }

            // Launch visible
            let driver = Self::launch_firefox(&profile_str, false).await?;
            session.driver = Some(driver);

            session.driver()?.goto(PROJECTIONLAB_URL).await?;

            info!("Please log in to ProjectionLab in the browser window.");
            session.wait_for_login().await?;
            info!("Login detected!");

            // Get API key while visible
            session.driver()?.goto(PLUGINS_SETTINGS_URL).await?;
            tokio::time::sleep(Duration::from_secs(2)).await;
            session.api_key = Some(session.extract_api_key().await?);
            info!("API key retrieved successfully!");

            // Quit visible, relaunch headless
            info!("Switching back to headless...");
            if let Some(driver) = session.driver.take() {
                let _ = driver.quit().await;
            }

            let driver = Self::launch_firefox(&profile_str, true).await?;
            session.driver = Some(driver);

            // Navigate to app
            session.driver()?.goto(PROJECTIONLAB_URL).await?;
            tokio::time::sleep(Duration::from_secs(2)).await;
        } else {
            info!("Session still valid, staying headless");

            // Get API key
            session.driver()?.goto(PLUGINS_SETTINGS_URL).await?;
            tokio::time::sleep(Duration::from_secs(2)).await;
            session.api_key = Some(session.extract_api_key().await?);
            info!("API key retrieved successfully!");

            // Navigate back to app
            session.driver()?.goto(PROJECTIONLAB_URL).await?;
            tokio::time::sleep(Duration::from_secs(2)).await;
        }

        info!("Ready for API calls (headless)");
        Ok(session)
    }

    /// Launch a Firefox instance (headless or visible)
    async fn launch_firefox(profile_path: &str, headless: bool) -> Result<WebDriver> {
        let mut caps = DesiredCapabilities::firefox();
        caps.add_arg("-profile")?;
        caps.add_arg(profile_path)?;
        if headless {
            caps.add_arg("-headless")?;
        }

        let driver = WebDriver::new(GECKODRIVER_URL, caps)
            .await
            .context("Failed to connect to GeckoDriver")?;

        Ok(driver)
    }

    /// Check if the current page indicates a logged-in state
    async fn check_logged_in(&self) -> bool {
        match self.driver().and_then(|d| Ok(d.current_url())) {
            Ok(fut) => match fut.await {
                Ok(url) => {
                    let url_str = url.as_str();
                    let logged_in = url_str.contains("app.projectionlab.com")
                        && !url_str.contains("/login")
                        && !url_str.contains("/auth")
                        && !url_str.contains("/signin");
                    info!("Login check: url={}, logged_in={}", url_str, logged_in);
                    logged_in
                }
                Err(e) => {
                    warn!("Failed to check URL: {}", e);
                    false
                }
            },
            Err(e) => {
                warn!("Driver not available: {}", e);
                false
            }
        }
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

    /// Get a reference to the WebDriver, or error if shutdown was called.
    fn driver(&self) -> Result<&WebDriver> {
        self.driver
            .as_ref()
            .context("Browser session has been shut down")
    }

    /// Waits for the user to log in by periodically checking the URL without navigating away
    async fn wait_for_login(&self) -> Result<()> {
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(LOGIN_WAIT_TIMEOUT_SECS);

        // Track consecutive failures
        let mut consecutive_failures = 0;
        const MAX_FAILURES: u32 = 3;

        // Configure exponential backoff for retries
        let mut backoff = ExponentialBackoffBuilder::new()
            .with_initial_interval(Duration::from_millis(500))
            .with_max_interval(Duration::from_secs(5))
            .with_multiplier(2.0)
            .with_max_elapsed_time(None) // We handle timeout separately
            .build();

        loop {
            // Check if login timeout has been reached
            if start.elapsed() > timeout {
                anyhow::bail!("Login timeout: User did not log in within {} seconds", LOGIN_WAIT_TIMEOUT_SECS);
            }

            // Check current URL without navigating
            match self.driver()?.current_url().await {
                Ok(url) => {
                    // Reset failure counter on success
                    consecutive_failures = 0;
                    backoff.reset();

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

                    // Not logged in yet, wait before checking again
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
                Err(e) => {
                    consecutive_failures += 1;
                    warn!(
                        "Error checking current URL (attempt {}/{}): {}",
                        consecutive_failures, MAX_FAILURES, e
                    );

                    if consecutive_failures >= MAX_FAILURES {
                        anyhow::bail!(
                            "Failed to check URL {} times consecutively. Browser may have crashed or become unresponsive.",
                            MAX_FAILURES
                        );
                    }

                    // Use exponential backoff before retrying
                    if let Some(wait_duration) = backoff.next_backoff() {
                        tokio::time::sleep(wait_duration).await;
                    }
                }
            }
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
            if let Ok(element) = self.driver()?.find(By::Css(selector)).await {
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

        let result = self.driver()?.execute(script, vec![]).await?;

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

        // Use execute_async for Promise-returning API methods.
        // The callback is injected as the last argument by WebDriver.
        let script = format!(
            r#"
            const callback = arguments[arguments.length - 1];
            window.projectionlabPluginAPI.{method}(...{args})
                .then(result => callback(result))
                .catch(err => callback({{"__error": err.message || String(err)}}));
            "#,
            method = method,
            args = args_json
        );

        info!("Executing plugin API call: {}", method);

        let result = self.driver()?.execute_async(&script, vec![]).await
            .context(format!("Failed to execute plugin API method: {}", method))?;

        let value = result.json().clone();

        // Check if the result is an error object from our catch handler
        if let Some(err_msg) = value.get("__error").and_then(|e| e.as_str()) {
            anyhow::bail!("Plugin API error in {}: {}", method, err_msg);
        }

        Ok(value)
    }

    /// Navigate the browser to a path within ProjectionLab (e.g., "/plan/abc123").
    /// Skips navigation if already on the same path.
    pub async fn navigate_to(&mut self, path: &str) -> Result<()> {
        if self.last_navigated_path.as_deref() == Some(path) {
            info!("Already on path: {}, skipping navigation", path);
            return Ok(());
        }
        let url = format!("{}{}", PROJECTIONLAB_URL, path);
        info!("Navigating to: {}", url);
        self.driver()?.goto(&url).await?;
        // Give the SPA time to route and render
        tokio::time::sleep(Duration::from_secs(2)).await;
        self.last_navigated_path = Some(path.to_string());
        Ok(())
    }

    /// Navigate back to the ProjectionLab home page.
    /// This ensures plugin API calls (exportData, etc.) run in a consistent context
    /// rather than on a plan-specific page which may alter the response.
    pub async fn navigate_to_home(&mut self) -> Result<()> {
        self.navigate_to("/").await
    }

    /// Execute arbitrary JavaScript in the browser and return the result as JSON
    pub async fn execute_js(&self, script: &str) -> Result<Value> {
        info!("Executing JS ({} chars)", script.len());
        let result = self.driver()?.execute(script, vec![]).await
            .context("Failed to execute JavaScript")?;
        Ok(result.json().clone())
    }

    /// Execute async JavaScript (with Promise support) and return the result
    pub async fn execute_js_async(&self, script: &str) -> Result<Value> {
        info!("Executing async JS ({} chars)", script.len());
        let result = self.driver()?.execute_async(script, vec![]).await
            .context("Failed to execute async JavaScript")?;
        let value = result.json().clone();
        if let Some(err_msg) = value.get("__error").and_then(|e| e.as_str()) {
            anyhow::bail!("JavaScript error: {}", err_msg);
        }
        Ok(value)
    }

    /// Wait for a DOM element matching the CSS selector to appear, with timeout
    pub async fn wait_for_element(&self, selector: &str, timeout_secs: u64) -> Result<()> {
        info!("Waiting for element: {} (timeout: {}s)", selector, timeout_secs);
        let script = format!(
            r#"
            const callback = arguments[arguments.length - 1];
            const timeout = {timeout} * 1000;
            const start = Date.now();
            const check = () => {{
                if (document.querySelector("{selector}")) {{
                    callback({{"found": true}});
                }} else if (Date.now() - start > timeout) {{
                    callback({{"__error": "Timeout waiting for element: {selector}"}});
                }} else {{
                    setTimeout(check, 250);
                }}
            }};
            check();
            "#,
            timeout = timeout_secs,
            selector = selector,
        );
        self.driver()?.execute_async(&script, vec![]).await
            .context(format!("Failed waiting for element: {}", selector))?;
        Ok(())
    }

    /// Gracefully shut down: quit the WebDriver session (closes Firefox), then kill GeckoDriver.
    pub async fn shutdown(&mut self) {
        info!("Shutting down browser session...");

        // Quit the WebDriver session — this closes the Firefox window
        if let Some(driver) = self.driver.take() {
            if let Err(e) = driver.quit().await {
                warn!("Error quitting WebDriver session: {}", e);
            } else {
                info!("Firefox closed");
            }
        }

        // Kill GeckoDriver if we started it
        if let Some(ref mut child) = self.geckodriver_process {
            if let Some(pid) = child.id() {
                info!("Stopping GeckoDriver (PID: {})...", pid);
                if let Err(e) = child.kill().await {
                    warn!("Error killing GeckoDriver: {}", e);
                } else {
                    info!("GeckoDriver stopped");
                }
            }
        }
        // Mark as None so Drop doesn't try again
        self.geckodriver_process = None;
    }
}

impl Drop for BrowserSession {
    fn drop(&mut self) {
        // If we started GeckoDriver and shutdown() wasn't called, best-effort cleanup.
        if let Some(child) = &mut self.geckodriver_process {
            if let Some(pid) = child.id() {
                info!("BrowserSession dropped - stopping GeckoDriver (PID: {})...", pid);
                if let Err(e) = child.start_kill() {
                    warn!("Error killing GeckoDriver: {}", e);
                }
            }
        }
    }
}
