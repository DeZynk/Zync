use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub username: String,
    pub uuid: String,
    pub account_type: AccountType,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AccountType {
    Offline,
    Microsoft,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherConfig {
    pub language: String,
    pub data_dir: String,
    pub default_java_path: Option<String>,
    pub min_memory_mb: u32,
    pub max_memory_mb: u32,
    pub jvm_args: String,
    pub max_concurrent_downloads: usize,
    pub window_width: u32,
    pub window_height: u32,
    pub close_on_launch: bool,
    pub enable_animations: bool,
    pub active_instance_id: Option<String>,
    pub active_account_id: Option<String>,
    pub accounts: Vec<Account>,
    pub ms_client_id: Option<String>,
}

const OPTIMIZED_JVM_ARGS: &str = "-XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=50 -XX:+UnlockExperimentalVMOptions -XX:+DisableExplicitGC -XX:G1NewSizePercent=20 -XX:G1ReservePercent=20 -XX:G1HeapRegionSize=8M";

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            data_dir: "ZyncData".to_string(),
            default_java_path: None,
            min_memory_mb: 1024,
            max_memory_mb: 4096,
            jvm_args: OPTIMIZED_JVM_ARGS.to_string(),
            max_concurrent_downloads: 16,
            window_width: 854,
            window_height: 480,
            close_on_launch: false,
            enable_animations: true,
            active_instance_id: None,
            active_account_id: None,
            accounts: Vec::new(),
            ms_client_id: None,
        }
    }
}

/// Returns the directory containing the running executable.
pub fn exe_dir() -> AppResult<PathBuf> {
    let exe = std::env::current_exe()
        .map_err(|e| AppError::Message(format!("Cannot locate executable: {e}")))?;
    exe.parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| AppError::Message("Executable has no parent directory".to_string()))
}

/// Path to the external config file stored next to the executable.
pub fn config_file_path() -> AppResult<PathBuf> {
    Ok(exe_dir()?.join("zync-config.json"))
}

/// Resolves the configured data directory (relative paths are resolved next to the executable).
pub fn resolve_data_dir(config: &LauncherConfig) -> AppResult<PathBuf> {
    let p = PathBuf::from(&config.data_dir);
    if p.is_absolute() {
        Ok(p)
    } else {
        Ok(exe_dir()?.join(p))
    }
}

/// Reads the external config file from disk on demand; creates default if absent.
pub fn read_config() -> AppResult<LauncherConfig> {
    let path = config_file_path()?;
    if !path.exists() {
        let default = LauncherConfig::default();
        write_config(&default)?;
        return Ok(default);
    }
    let content = fs::read_to_string(&path)?;
    let mut parsed: LauncherConfig = serde_json::from_str(&content)?;
    // Upgrade legacy 32M G1HeapRegionSize which caused GC micro-stutters on 4GB heaps
    if parsed.jvm_args.contains("-XX:G1HeapRegionSize=32M") {
        parsed.jvm_args = OPTIMIZED_JVM_ARGS.to_string();
        let _ = write_config(&parsed);
    }
    Ok(parsed)
}

/// Writes the config file to disk next to the executable.
pub fn write_config(config: &LauncherConfig) -> AppResult<()> {
    let path = config_file_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(config)?;
    fs::write(&path, json)?;
    Ok(())
}

#[tauri::command]
pub fn load_config() -> AppResult<LauncherConfig> {
    read_config()
}

#[tauri::command]
pub fn save_config(config: LauncherConfig) -> AppResult<LauncherConfig> {
    write_config(&config)?;
    Ok(config)
}

#[tauri::command]
pub fn get_config_path() -> AppResult<String> {
    Ok(config_file_path()?.to_string_lossy().to_string())
}

#[tauri::command]
pub fn open_config_folder() -> AppResult<()> {
    let dir = exe_dir()?;
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer").arg(dir).spawn()?;
    }
    Ok(())
}
