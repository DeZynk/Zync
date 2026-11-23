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

fn default_view_style() -> String {
    "grid".to_string()
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
    #[serde(default = "default_view_style")]
    pub instances_view_style: String,
    #[serde(default)]
    pub sidebar_collapsed: bool,
    pub active_instance_id: Option<String>,
    pub active_account_id: Option<String>,
    /// Populated at runtime from DPAPI-encrypted `accounts.dat`; omitted when saving `zync-config.json`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
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
            instances_view_style: "grid".to_string(),
            sidebar_collapsed: false,
            active_instance_id: None,
            active_account_id: None,
            accounts: Vec::new(),
            ms_client_id: None,
        }
    }
}

#[cfg(target_os = "windows")]
mod dpapi {
    use crate::error::{AppError, AppResult};
    use std::ffi::c_void;
    use std::ptr;

    const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x01;

    #[repr(C)]
    struct DataBlob {
        cb_data: u32,
        pb_data: *mut u8,
    }

    #[link(name = "crypt32")]
    extern "system" {
        fn CryptProtectData(
            p_data_in: *const DataBlob,
            sz_data_descr: *const u16,
            p_optional_entropy: *const DataBlob,
            pv_reserved: *mut c_void,
            p_prompt_struct: *mut c_void,
            dw_flags: u32,
            p_data_out: *mut DataBlob,
        ) -> i32;

        fn CryptUnprotectData(
            p_data_in: *const DataBlob,
            ppsz_data_descr: *mut *mut u16,
            p_optional_entropy: *const DataBlob,
            pv_reserved: *mut c_void,
            p_prompt_struct: *mut c_void,
            dw_flags: u32,
            p_data_out: *mut DataBlob,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(hmem: *mut c_void) -> *mut c_void;
    }

    pub fn encrypt(plain: &[u8]) -> AppResult<Vec<u8>> {
        let mut input = plain.to_vec();
        let in_blob = DataBlob {
            cb_data: input.len() as u32,
            pb_data: input.as_mut_ptr(),
        };
        let mut out_blob = DataBlob {
            cb_data: 0,
            pb_data: ptr::null_mut(),
        };

        let ok = unsafe {
            CryptProtectData(
                &in_blob,
                ptr::null(),
                ptr::null(),
                ptr::null_mut(),
                ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out_blob,
            )
        };
        if ok == 0 {
            return Err(AppError::Message(
                "Windows DPAPI failed to encrypt account data".to_string(),
            ));
        }

        let result = unsafe {
            let slice = std::slice::from_raw_parts(out_blob.pb_data, out_blob.cb_data as usize);
            let v = slice.to_vec();
            LocalFree(out_blob.pb_data as *mut c_void);
            v
        };
        Ok(result)
    }

    pub fn decrypt(cipher: &[u8]) -> AppResult<Vec<u8>> {
        let mut input = cipher.to_vec();
        let in_blob = DataBlob {
            cb_data: input.len() as u32,
            pb_data: input.as_mut_ptr(),
        };
        let mut out_blob = DataBlob {
            cb_data: 0,
            pb_data: ptr::null_mut(),
        };

        let ok = unsafe {
            CryptUnprotectData(
                &in_blob,
                ptr::null_mut(),
                ptr::null(),
                ptr::null_mut(),
                ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out_blob,
            )
        };
        if ok == 0 {
            return Err(AppError::Message(
                "Windows DPAPI failed to decrypt account data".to_string(),
            ));
        }

        let result = unsafe {
            let slice = std::slice::from_raw_parts(out_blob.pb_data, out_blob.cb_data as usize);
            let v = slice.to_vec();
            LocalFree(out_blob.pb_data as *mut c_void);
            v
        };
        Ok(result)
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

fn accounts_store_path(config: &LauncherConfig) -> AppResult<PathBuf> {
    Ok(resolve_data_dir(config)?.join("accounts.dat"))
}

fn read_encrypted_accounts(config: &LauncherConfig) -> AppResult<Option<Vec<Account>>> {
    let path = accounts_store_path(config)?;
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path)?;
    #[cfg(target_os = "windows")]
    let plain = dpapi::decrypt(&bytes)?;
    #[cfg(not(target_os = "windows"))]
    let plain = bytes;
    let accounts: Vec<Account> = serde_json::from_slice(&plain)?;
    Ok(Some(accounts))
}

fn write_encrypted_accounts(config: &LauncherConfig, accounts: &[Account]) -> AppResult<()> {
    let path = accounts_store_path(config)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json_bytes = serde_json::to_vec(accounts)?;
    #[cfg(target_os = "windows")]
    let out_bytes = dpapi::encrypt(&json_bytes)?;
    #[cfg(not(target_os = "windows"))]
    let out_bytes = json_bytes;
    fs::write(&path, out_bytes)?;
    Ok(())
}

/// Reads the external config file from disk on demand; creates default if absent.
/// Account credentials are stored separately in Windows DPAPI-encrypted `accounts.dat`.
pub fn read_config() -> AppResult<LauncherConfig> {
    let path = config_file_path()?;
    if !path.exists() {
        let default = LauncherConfig::default();
        write_config(&default)?;
        return Ok(default);
    }
    let content = fs::read_to_string(&path)?;
    let mut parsed: LauncherConfig = serde_json::from_str(&content)?;
    let mut needs_resave = false;

    // Upgrade legacy 32M G1HeapRegionSize which caused GC micro-stutters on 4GB heaps
    if parsed.jvm_args.contains("-XX:G1HeapRegionSize=32M") {
        parsed.jvm_args = OPTIMIZED_JVM_ARGS.to_string();
        needs_resave = true;
    }

    // Load DPAPI-encrypted accounts; auto-migrate legacy plaintext accounts out of zync-config.json
    match read_encrypted_accounts(&parsed) {
        Ok(Some(encrypted_accounts)) => {
            if !parsed.accounts.is_empty() {
                // Strip any remaining legacy plaintext accounts from zync-config.json
                needs_resave = true;
            }
            parsed.accounts = encrypted_accounts;
        }
        Ok(None) => {
            if !parsed.accounts.is_empty() {
                // Migrate legacy plaintext accounts to DPAPI-encrypted accounts.dat
                write_encrypted_accounts(&parsed, &parsed.accounts)?;
                needs_resave = true;
            }
        }
        Err(_) => {
            // If accounts.dat cannot be decrypted (e.g. copied from another PC), start with empty accounts
            parsed.accounts = Vec::new();
        }
    }

    if needs_resave {
        let _ = write_config(&parsed);
    }

    Ok(parsed)
}

/// Writes non-sensitive settings to `zync-config.json` and stores accounts encrypted in `accounts.dat`.
pub fn write_config(config: &LauncherConfig) -> AppResult<()> {
    let path = config_file_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // 1. Persist accounts encrypted with Windows DPAPI in `<dataDir>/accounts.dat`
    write_encrypted_accounts(config, &config.accounts)?;

    // 2. Write `zync-config.json` WITHOUT any account tokens or credentials
    let mut sanitized = config.clone();
    sanitized.accounts = Vec::new();
    let json = serde_json::to_string_pretty(&sanitized)?;
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

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::dpapi;

    #[test]
    fn test_dpapi_encrypt_decrypt_roundtrip() {
        let secret = b"{\"accessToken\":\"super-secret-token\"}";
        let encrypted = dpapi::encrypt(secret).expect("encrypt failed");
        assert_ne!(encrypted.as_slice(), secret);
        let decrypted = dpapi::decrypt(&encrypted).expect("decrypt failed");
        assert_eq!(decrypted.as_slice(), secret);
    }
}
