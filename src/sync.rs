use crate::browser::BrowserSession;
use crate::models::{parse_export, FullExport};
use anyhow::{Context, Result};
use serde_json::{json, Map, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Cache time-to-live in seconds
const CACHE_TTL_SECS: u64 = 30;

/// How long to wait for the browser session to initialize before giving up
const BROWSER_READY_TIMEOUT_SECS: u64 = 60;

type SharedBrowser = Arc<tokio::sync::Mutex<Option<BrowserSession>>>;

/// Manages synchronization and caching of ProjectionLab data.
///
/// Reads go through a short-lived cache of the `exportData` result, kept both
/// as raw JSON (the source of truth for writes, so round trips are lossless)
/// and as a parsed `FullExport` (for convenient typed reads).
pub struct SyncManager {
    browser: SharedBrowser,
    cached_data: Arc<RwLock<Option<CachedData>>>,
}

struct CachedData {
    raw: Arc<Value>,
    data: Arc<FullExport>,
    cached_at: Instant,
}

impl SyncManager {
    pub fn new(browser: SharedBrowser) -> Self {
        Self {
            browser,
            cached_data: Arc::new(RwLock::new(None)),
        }
    }

    /// Parsed export data, fetched from ProjectionLab if the cache is stale.
    pub async fn get_data(&self) -> Result<Arc<FullExport>> {
        Ok(self.get_cached().await?.1)
    }

    /// Raw export JSON (an owned copy, safe to mutate and write back).
    pub async fn get_raw(&self) -> Result<Value> {
        Ok((*self.get_cached().await?.0).clone())
    }

    async fn get_cached(&self) -> Result<(Arc<Value>, Arc<FullExport>)> {
        {
            let cache = self.cached_data.read().await;
            if let Some(cached) = cache.as_ref() {
                if cached.cached_at.elapsed() < Duration::from_secs(CACHE_TTL_SECS) {
                    debug!("Returning cached data (age: {:?})", cached.cached_at.elapsed());
                    return Ok((cached.raw.clone(), cached.data.clone()));
                }
                debug!("Cache is stale (age: {:?}), will refresh", cached.cached_at.elapsed());
            }
        }
        info!("Fetching data from ProjectionLab...");
        self.fetch_and_store().await
    }

    /// Force a refresh of the cached data
    pub async fn refresh(&self) -> Result<Arc<FullExport>> {
        info!("Force refreshing data from ProjectionLab...");
        Ok(self.fetch_and_store().await?.1)
    }

    async fn fetch_and_store(&self) -> Result<(Arc<Value>, Arc<FullExport>)> {
        let raw = self
            .call_api("exportData", vec![])
            .await
            .context("Failed to export data from ProjectionLab")?;
        let data = parse_export(&raw)?;
        let (raw, data) = (Arc::new(raw), Arc::new(data));
        *self.cached_data.write().await = Some(CachedData {
            raw: raw.clone(),
            data: data.clone(),
            cached_at: Instant::now(),
        });
        info!("Successfully fetched data from ProjectionLab");
        Ok((raw, data))
    }

    /// Invalidate the cache (call after write operations)
    pub async fn invalidate(&self) {
        debug!("Invalidating cache");
        *self.cached_data.write().await = None;
    }

    /// Get cache age for debugging
    pub async fn cache_age(&self) -> Option<Duration> {
        let cache = self.cached_data.read().await;
        cache.as_ref().map(|c| c.cached_at.elapsed())
    }

    /// Wait for the browser session to be initialized (it starts in a background task).
    async fn wait_for_browser(&self) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(BROWSER_READY_TIMEOUT_SECS);
        loop {
            if self.browser.lock().await.is_some() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                anyhow::bail!(
                    "Browser session not initialized after {}s — is Firefox/GeckoDriver available?",
                    BROWSER_READY_TIMEOUT_SECS
                );
            }
            debug!("Waiting for browser session to initialize...");
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    async fn lock_browser(&self) -> Result<tokio::sync::MutexGuard<'_, Option<BrowserSession>>> {
        self.wait_for_browser().await?;
        Ok(self.browser.lock().await)
    }

    /// Call a Plugin API method (read-only; does not touch the cache).
    pub async fn call_api(&self, method: &str, args: Vec<Value>) -> Result<Value> {
        let guard = self.lock_browser().await?;
        let browser = guard.as_ref().context("Browser session not initialized")?;
        browser.call_plugin_api(method, args).await
    }

    /// Run async JavaScript in the browser on whatever page is currently loaded.
    pub async fn run_js_async(&self, script: &str) -> Result<Value> {
        let guard = self.lock_browser().await?;
        let browser = guard.as_ref().context("Browser session not initialized")?;
        browser.execute_js_async(script).await
    }

    // ---- Writes (each invalidates the cache) ----

    /// Replace the Current Finances document.
    ///
    /// Note: ProjectionLab V5 *merges* submitted accounts over the stored ones,
    /// so omitting an account does not delete it (archive it instead).
    pub async fn restore_current_finances(&self, current_finances: Value) -> Result<()> {
        info!("Restoring Current Finances in ProjectionLab...");
        self.write("restoreCurrentFinances", vec![current_finances], Map::new())
            .await
            .context("Failed to restore current finances")
    }

    /// Replace the full list of plans. Plans missing from the array are deleted.
    pub async fn restore_plans(&self, plans: Value) -> Result<()> {
        anyhow::ensure!(plans.is_array(), "restorePlans expects an array of plans");
        info!("Restoring plans in ProjectionLab...");
        self.write("restorePlans", vec![plans], Map::new())
            .await
            .context("Failed to restore plans")
    }

    /// Replace the settings document.
    pub async fn restore_settings(&self, settings: Value) -> Result<()> {
        info!("Restoring settings in ProjectionLab...");
        self.write("restoreSettings", vec![settings], Map::new())
            .await
            .context("Failed to restore settings")
    }

    /// Assign fields on a Current Finances account (`updateAccount`). With
    /// `force`, fields not already present on the account may be added.
    pub async fn update_account(&self, account_id: &str, data: Value, force: bool) -> Result<()> {
        let mut options = Map::new();
        if force {
            options.insert("force".into(), Value::Bool(true));
        }
        self.write("updateAccount", vec![json!(account_id), data], options)
            .await
            .with_context(|| format!("Failed to update account {}", account_id))
    }

    /// Update an account's current values (`updateAccountValues`). Supported
    /// fields: `balance` (accounts, debts), `costBasis` (accounts that track it),
    /// `value` / `purchasePrice` (real assets). ProjectionLab records account
    /// history automatically for these edits.
    pub async fn update_account_values(&self, account_id: &str, values: Value) -> Result<()> {
        let result = {
            let guard = self.lock_browser().await?;
            let browser = guard.as_ref().context("Browser session not initialized")?;
            let mut params = Map::new();
            params.insert("accountId".into(), json!(account_id));
            params.insert("values".into(), values);
            match browser
                .call_plugin_api_single_object("updateAccountValues", params)
                .await
            {
                Ok(_) => browser.flush_pending_saves().await,
                Err(e) => Err(e),
            }
        };
        self.invalidate().await;
        result
            .map(|_| ())
            .with_context(|| format!("Failed to update values for account {}", account_id))
    }

    async fn write(&self, method: &str, args: Vec<Value>, options: Map<String, Value>) -> Result<()> {
        let result = {
            let guard = self.lock_browser().await?;
            let browser = guard.as_ref().context("Browser session not initialized")?;
            match browser
                .call_plugin_api_with_options(method, args, options)
                .await
            {
                // Only report success once ProjectionLab has persisted the change.
                Ok(_) => browser.flush_pending_saves().await,
                Err(e) => Err(e),
            }
        };
        // Invalidate even on failure: a partial write may have happened.
        self.invalidate().await;
        result.map(|_| ())
    }
}
