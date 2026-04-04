use crate::browser::BrowserSession;
use crate::models::FullExport;
use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Cache time-to-live in seconds
const CACHE_TTL_SECS: u64 = 30;

/// Manages synchronization and caching of ProjectionLab data
pub struct SyncManager {
    browser: Arc<tokio::sync::Mutex<Option<BrowserSession>>>,
    cached_data: Arc<RwLock<Option<CachedData>>>,
}

struct CachedData {
    data: FullExport,
    cached_at: Instant,
}

impl SyncManager {
    pub fn new(browser: Arc<tokio::sync::Mutex<Option<BrowserSession>>>) -> Self {
        Self {
            browser,
            cached_data: Arc::new(RwLock::new(None)),
        }
    }

    /// Get the current data, fetching from ProjectionLab if cache is stale
    pub async fn get_data(&self) -> Result<FullExport> {
        // Check if cache is valid
        {
            let cache = self.cached_data.read().await;
            if let Some(cached) = cache.as_ref() {
                if cached.cached_at.elapsed() < Duration::from_secs(CACHE_TTL_SECS) {
                    debug!("Returning cached data (age: {:?})", cached.cached_at.elapsed());
                    return Ok(cached.data.clone());
                } else {
                    debug!("Cache is stale (age: {:?}), will refresh", cached.cached_at.elapsed());
                }
            }
        }

        // Cache miss or stale, fetch new data
        info!("Fetching data from ProjectionLab...");
        let data = self.fetch_data().await?;

        // Update cache
        {
            let mut cache = self.cached_data.write().await;
            *cache = Some(CachedData {
                data: data.clone(),
                cached_at: Instant::now(),
            });
        }

        Ok(data)
    }

    /// Force a refresh of the cached data
    pub async fn refresh(&self) -> Result<FullExport> {
        info!("Force refreshing data from ProjectionLab...");

        let data = self.fetch_data().await?;

        // Update cache
        {
            let mut cache = self.cached_data.write().await;
            *cache = Some(CachedData {
                data: data.clone(),
                cached_at: Instant::now(),
            });
        }

        Ok(data)
    }

    /// Invalidate the cache (call after write operations)
    pub async fn invalidate(&self) {
        debug!("Invalidating cache");
        let mut cache = self.cached_data.write().await;
        *cache = None;
    }

    /// Fetch data from ProjectionLab via the browser
    async fn fetch_data(&self) -> Result<FullExport> {
        let browser_guard = self.browser.lock().await;
        let browser = browser_guard
            .as_ref()
            .context("Browser session not initialized")?;

        let result = browser
            .call_plugin_api("exportData", vec![])
            .await
            .context("Failed to export data from ProjectionLab")?;

        let export: FullExport = serde_json::from_value(result)
            .context("Failed to parse exported data")?;

        info!("Successfully fetched data from ProjectionLab");
        Ok(export)
    }

    /// Update data in ProjectionLab and invalidate cache
    ///
    /// This is used by domain tools to push changes back to ProjectionLab
    pub async fn update_current_finances(&self, new_finances: serde_json::Value) -> Result<()> {
        info!("Updating current finances in ProjectionLab...");

        let browser_guard = self.browser.lock().await;
        let browser = browser_guard
            .as_ref()
            .context("Browser session not initialized")?;

        browser
            .call_plugin_api("restoreCurrentFinances", vec![new_finances])
            .await
            .context("Failed to restore current finances")?;

        // Invalidate cache after write
        drop(browser_guard);
        self.invalidate().await;

        info!("Successfully updated current finances");
        Ok(())
    }

    /// Update plans in ProjectionLab and invalidate cache
    pub async fn update_plans(&self, new_plans: serde_json::Value) -> Result<()> {
        info!("Updating plans in ProjectionLab...");

        let browser_guard = self.browser.lock().await;
        let browser = browser_guard
            .as_ref()
            .context("Browser session not initialized")?;

        browser
            .call_plugin_api("restorePlans", vec![new_plans])
            .await
            .context("Failed to restore plans")?;

        // Invalidate cache after write
        drop(browser_guard);
        self.invalidate().await;

        info!("Successfully updated plans");
        Ok(())
    }

    /// Update progress in ProjectionLab and invalidate cache
    pub async fn update_progress(&self, new_progress: serde_json::Value) -> Result<()> {
        info!("Updating progress in ProjectionLab...");

        let browser_guard = self.browser.lock().await;
        let browser = browser_guard
            .as_ref()
            .context("Browser session not initialized")?;

        browser
            .call_plugin_api("restoreProgress", vec![new_progress])
            .await
            .context("Failed to restore progress")?;

        // Invalidate cache after write
        drop(browser_guard);
        self.invalidate().await;

        info!("Successfully updated progress");
        Ok(())
    }

    /// Update settings in ProjectionLab and invalidate cache
    pub async fn update_settings(&self, new_settings: serde_json::Value) -> Result<()> {
        info!("Updating settings in ProjectionLab...");

        let browser_guard = self.browser.lock().await;
        let browser = browser_guard
            .as_ref()
            .context("Browser session not initialized")?;

        browser
            .call_plugin_api("restoreSettings", vec![new_settings])
            .await
            .context("Failed to restore settings")?;

        // Invalidate cache after write
        drop(browser_guard);
        self.invalidate().await;

        info!("Successfully updated settings");
        Ok(())
    }

    /// Get cache age for debugging
    pub async fn cache_age(&self) -> Option<Duration> {
        let cache = self.cached_data.read().await;
        cache.as_ref().map(|c| c.cached_at.elapsed())
    }
}
