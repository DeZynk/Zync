use crate::config::{read_config, write_config, Account, AccountType};
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

const DEFAULT_LIVE_CLIENT_ID: &str = "00000000402b5328";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCodeResponse {
    pub user_code: String,
    pub device_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
    pub message: String,
}

#[derive(Deserialize)]
struct MsDeviceCodeRaw {
    user_code: String,
    device_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Deserialize)]
struct MsTokenRaw {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: i64,
}

#[derive(Deserialize)]
struct MsTokenErr {
    error: String,
    error_description: Option<String>,
}

fn open_url_in_browser(url: &str) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }
}

#[tauri::command]
pub fn add_offline_account(username: String) -> AppResult<Account> {
    let trimmed = username.trim();
    if trimmed.is_empty() || trimmed.len() > 16 {
        return Err(AppError::Message(
            "Username must be 1 to 16 characters".to_string(),
        ));
    }

    let uuid = Uuid::new_v3(
        &Uuid::NAMESPACE_OID,
        format!("OfflinePlayer:{trimmed}").as_bytes(),
    )
    .simple()
    .to_string();

    let account = Account {
        id: Uuid::new_v4().to_string(),
        username: trimmed.to_string(),
        uuid,
        account_type: AccountType::Offline,
        access_token: None,
        refresh_token: None,
        expires_at: None,
    };

    let mut cfg = read_config()?;
    cfg.accounts.push(account.clone());
    cfg.active_account_id = Some(account.id.clone());
    write_config(&cfg)?;

    Ok(account)
}

#[tauri::command]
pub fn remove_account(account_id: String) -> AppResult<()> {
    let mut cfg = read_config()?;
    cfg.accounts.retain(|a| a.id != account_id);
    if cfg.active_account_id.as_deref() == Some(account_id.as_str()) {
        cfg.active_account_id = cfg.accounts.first().map(|a| a.id.clone());
    }
    write_config(&cfg)?;
    Ok(())
}

#[tauri::command]
pub fn set_active_account(account_id: String) -> AppResult<()> {
    let mut cfg = read_config()?;
    if !cfg.accounts.iter().any(|a| a.id == account_id) {
        return Err(AppError::Message("Account not found".to_string()));
    }
    cfg.active_account_id = Some(account_id);
    write_config(&cfg)?;
    Ok(())
}

#[tauri::command]
pub async fn start_microsoft_login(client_id: Option<String>) -> AppResult<DeviceCodeResponse> {
    let custom_cid = client_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let client = reqwest::Client::new();
    let resp: MsDeviceCodeRaw = if let Some(cid) = custom_cid {
        client
            .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode")
            .form(&[
                ("client_id", cid),
                ("scope", "XboxLive.signin offline_access"),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?
    } else {
        client
            .post("https://login.live.com/oauth20_connect.srf")
            .form(&[
                ("client_id", DEFAULT_LIVE_CLIENT_ID),
                ("scope", "service::user.auth.xboxlive.com::MBI_SSL"),
                ("response_type", "device_code"),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?
    };

    // Automatically open verification URI in user's browser
    open_url_in_browser(&resp.verification_uri);

    let msg = resp.message.unwrap_or_else(|| {
        format!(
            "Browser opened to {}. Paste the code below to sign in with your Microsoft account.",
            resp.verification_uri
        )
    });

    Ok(DeviceCodeResponse {
        user_code: resp.user_code,
        device_code: resp.device_code,
        verification_uri: resp.verification_uri,
        expires_in: resp.expires_in,
        interval: resp.interval,
        message: msg,
    })
}

#[tauri::command]
pub async fn complete_microsoft_login(
    client_id: Option<String>,
    device_code: String,
) -> AppResult<Option<Account>> {
    let custom_cid = client_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let use_live = custom_cid.is_none();
    let cid = custom_cid.unwrap_or(DEFAULT_LIVE_CLIENT_ID);

    let client = reqwest::Client::new();
    let token_url = if use_live {
        "https://login.live.com/oauth20_token.srf"
    } else {
        "https://login.microsoftonline.com/consumers/oauth2/v2.0/token"
    };

    let token_resp = client
        .post(token_url)
        .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("client_id", cid),
            ("device_code", device_code.as_str()),
        ])
        .send()
        .await?;

    if !token_resp.status().is_success() {
        let err_body = token_resp.json::<MsTokenErr>().await?;
        if err_body.error == "authorization_pending" || err_body.error == "slow_down" {
            return Ok(None);
        }
        return Err(AppError::Message(
            err_body.error_description.unwrap_or(err_body.error),
        ));
    }

    let ms_token: MsTokenRaw = token_resp.json().await?;

    // 1. Xbox Live Auth (Live MBI_SSL token uses "t=", Azure OAuth token uses "d=")
    let rps_ticket = if use_live {
        format!("t={}", ms_token.access_token)
    } else {
        format!("d={}", ms_token.access_token)
    };

    let xbl_resp: serde_json::Value = client
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&serde_json::json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": rps_ticket
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let xbl_token = xbl_resp["Token"]
        .as_str()
        .ok_or_else(|| AppError::Message("Missing Xbox Live token".to_string()))?;
    let user_hash = xbl_resp["DisplayClaims"]["xui"][0]["uhs"]
        .as_str()
        .ok_or_else(|| AppError::Message("Missing Xbox user hash".to_string()))?;

    // 2. XSTS Auth
    let xsts_resp: serde_json::Value = client
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&serde_json::json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [xbl_token]
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let xsts_token = xsts_resp["Token"]
        .as_str()
        .ok_or_else(|| AppError::Message("Missing XSTS token".to_string()))?;

    // 3. Minecraft Services Auth
    let mc_resp: serde_json::Value = client
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&serde_json::json!({
            "identityToken": format!("XBL3.0 x={user_hash};{xsts_token}")
        }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let mc_access_token = mc_resp["access_token"]
        .as_str()
        .ok_or_else(|| AppError::Message("Missing Minecraft access token".to_string()))?
        .to_string();

    // 4. Minecraft Profile
    let profile_resp = client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(&mc_access_token)
        .send()
        .await?;

    if profile_resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(AppError::Message(
            "This Microsoft account does not have a Minecraft: Java Edition profile (404). Make sure your browser signed into the Microsoft account that owns Java Edition, or create your Java profile name on minecraft.net first.".to_string(),
        ));
    }

    let profile: serde_json::Value = profile_resp.error_for_status()?.json().await?;

    let username = profile["name"]
        .as_str()
        .ok_or_else(|| {
            AppError::Message("This Microsoft account does not own Minecraft".to_string())
        })?
        .to_string();
    let uuid = profile["id"]
        .as_str()
        .ok_or_else(|| AppError::Message("Missing Minecraft UUID".to_string()))?
        .to_string();

    let account = Account {
        id: Uuid::new_v4().to_string(),
        username,
        uuid,
        account_type: AccountType::Microsoft,
        access_token: Some(mc_access_token),
        refresh_token: ms_token.refresh_token,
        expires_at: Some(chrono::Utc::now().timestamp() + ms_token.expires_in),
    };

    let mut cfg = read_config()?;
    cfg.accounts.retain(|a| a.uuid != account.uuid);
    cfg.accounts.push(account.clone());
    cfg.active_account_id = Some(account.id.clone());
    write_config(&cfg)?;

    Ok(Some(account))
}
