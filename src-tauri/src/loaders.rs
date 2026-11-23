use crate::error::{AppError, AppResult};
use crate::instances::LoaderType;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderVersionInfo {
    pub version: String,
    pub stable: bool,
}

#[derive(Deserialize)]
struct MetaLoaderEntry {
    loader: MetaLoaderInner,
}

#[derive(Deserialize)]
struct MetaLoaderInner {
    version: String,
    #[serde(default)]
    stable: bool,
}

#[derive(Deserialize)]
struct NeoForgeVersionsResp {
    versions: Vec<String>,
}

#[tauri::command]
pub async fn list_loader_versions(
    loader: LoaderType,
    mc_version: String,
) -> AppResult<Vec<LoaderVersionInfo>> {
    let client = reqwest::Client::builder()
        .user_agent("ZyncLauncher/0.1.0")
        .build()?;

    match loader {
        LoaderType::Vanilla => Ok(Vec::new()),
        LoaderType::Fabric => {
            let url = format!("https://meta.fabricmc.net/v2/versions/loader/{mc_version}");
            let entries: Vec<MetaLoaderEntry> = client
                .get(&url)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            Ok(entries
                .into_iter()
                .map(|e| LoaderVersionInfo {
                    version: e.loader.version,
                    stable: e.loader.stable,
                })
                .collect())
        }
        LoaderType::Quilt => {
            let url = format!("https://meta.quiltmc.org/v3/versions/loader/{mc_version}");
            let entries: Vec<MetaLoaderEntry> = client
                .get(&url)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            Ok(entries
                .into_iter()
                .map(|e| {
                    let stable = !e.loader.version.contains("beta");
                    LoaderVersionInfo {
                        version: e.loader.version,
                        stable,
                    }
                })
                .collect())
        }
        LoaderType::NeoForge => {
            let url =
                "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";
            let resp: NeoForgeVersionsResp = client
                .get(url)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let parts: Vec<&str> = mc_version.split('.').collect();
            let prefix = if parts.len() >= 3 {
                format!("{}.{}.", parts[1], parts[2])
            } else if parts.len() == 2 {
                format!("{}.0.", parts[1])
            } else {
                mc_version.clone()
            };

            let mut list: Vec<LoaderVersionInfo> = resp
                .versions
                .into_iter()
                .filter(|v| v.starts_with(&prefix))
                .map(|v| LoaderVersionInfo {
                    stable: !v.contains("beta") && !v.contains("alpha"),
                    version: v,
                })
                .collect();
            list.reverse();
            Ok(list)
        }
        LoaderType::Forge => {
            let url = "https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json";
            let metadata: HashMap<String, Vec<String>> = client
                .get(url)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;

            let Some(versions) = metadata.get(&mc_version) else {
                return Ok(Vec::new());
            };

            let prefix = format!("{mc_version}-");
            let mut list: Vec<LoaderVersionInfo> = versions
                .iter()
                .map(|raw| {
                    let stripped = raw.strip_prefix(&prefix).unwrap_or(raw).to_string();
                    LoaderVersionInfo {
                        version: stripped,
                        stable: true,
                    }
                })
                .collect();
            list.reverse();
            Ok(list)
        }
    }
}

pub async fn fetch_fabric_or_quilt_profile(
    loader: &LoaderType,
    mc_version: &str,
    loader_version: &str,
) -> AppResult<Value> {
    let client = reqwest::Client::builder()
        .user_agent("ZyncLauncher/0.1.0")
        .build()?;

    let url = match loader {
        LoaderType::Fabric => format!(
            "https://meta.fabricmc.net/v2/versions/loader/{mc_version}/{loader_version}/profile/json"
        ),
        LoaderType::Quilt => format!(
            "https://meta.quiltmc.org/v3/versions/loader/{mc_version}/{loader_version}/profile/json"
        ),
        _ => return Err(AppError::Message("Not a Fabric/Quilt loader".to_string())),
    };

    let json: Value = client
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(json)
}

fn find_installed_loader_json(
    mc_dir: &Path,
    loader: &LoaderType,
    mc_version: &str,
    loader_version: &str,
) -> Option<PathBuf> {
    let versions_dir = mc_dir.join("versions");
    let candidates = match loader {
        LoaderType::NeoForge => vec![format!("neoforge-{loader_version}")],
        LoaderType::Forge => vec![
            format!("{mc_version}-forge-{loader_version}"),
            format!("{mc_version}-forge{mc_version}-{loader_version}"),
            format!("{mc_version}-Forge{loader_version}"),
        ],
        _ => Vec::new(),
    };

    for vid in candidates {
        let p = versions_dir.join(&vid).join(format!("{vid}.json"));
        if p.exists() {
            return Some(p);
        }
    }

    // Fallback scan for any version folder matching both mc_version and loader_version
    if let Ok(entries) = fs::read_dir(&versions_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let lower = name.to_lowercase();
            let is_match = match loader {
                LoaderType::NeoForge => lower.contains("neoforge") && name.contains(loader_version),
                LoaderType::Forge => {
                    lower.contains("forge")
                        && !lower.contains("neoforge")
                        && name.contains(loader_version)
                }
                _ => false,
            };
            if is_match {
                let p = entry.path().join(format!("{name}.json"));
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }

    None
}

/// Ensures Forge or NeoForge is installed directly into the instance's `.minecraft` directory.
pub async fn ensure_forge_or_neoforge_installed(
    mc_dir: &Path,
    loader: &LoaderType,
    mc_version: &str,
    loader_version: &str,
    java_exe: &str,
) -> AppResult<PathBuf> {
    if let Some(existing) = find_installed_loader_json(mc_dir, loader, mc_version, loader_version) {
        return Ok(existing);
    }

    let version_id = match loader {
        LoaderType::NeoForge => format!("neoforge-{loader_version}"),
        LoaderType::Forge => format!("{mc_version}-forge-{loader_version}"),
        _ => return Err(AppError::Message("Not a Forge/NeoForge loader".to_string())),
    };

    let profiles_path = mc_dir.join("launcher_profiles.json");
    if !profiles_path.exists() {
        fs::write(&profiles_path, r#"{"profiles":{}}"#)?;
    }

    let installer_url = match loader {
        LoaderType::NeoForge => format!(
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/{loader_version}/neoforge-{loader_version}-installer.jar"
        ),
        LoaderType::Forge => format!(
            "https://maven.minecraftforge.net/net/minecraftforge/forge/{mc_version}-{loader_version}/forge-{mc_version}-{loader_version}-installer.jar"
        ),
        _ => unreachable!(),
    };

    let client = reqwest::Client::builder()
        .user_agent("ZyncLauncher/0.1.0")
        .build()?;
    let bytes = client
        .get(&installer_url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;

    let tmp_dir = mc_dir.join("tmp");
    fs::create_dir_all(&tmp_dir)?;
    let installer_path = tmp_dir.join(format!("{version_id}-installer.jar"));
    fs::write(&installer_path, &bytes)?;

    let mut cmd = std::process::Command::new(java_exe);
    cmd.arg("-jar")
        .arg(&installer_path)
        .arg("--installClient")
        .arg(mc_dir);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd.output()?;
    let _ = fs::remove_file(&installer_path);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if stderr.trim().is_empty() { stdout } else { stderr };
        return Err(AppError::Message(format!(
            "Mod loader installer failed: {detail}"
        )));
    }

    find_installed_loader_json(mc_dir, loader, mc_version, loader_version).ok_or_else(|| {
        AppError::Message(format!(
            "Installer finished but version JSON for {version_id} was not found in {}",
            mc_dir.join("versions").display()
        ))
    })
}
