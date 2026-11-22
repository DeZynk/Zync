use crate::config::{read_config, resolve_data_dir, write_config};
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LoaderType {
    Vanilla,
    Fabric,
    Quilt,
    Forge,
    NeoForge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInstance {
    pub id: String,
    pub name: String,
    pub mc_version: String,
    pub loader: LoaderType,
    pub loader_version: Option<String>,
    pub java_path: Option<String>,
    pub min_memory_mb: Option<u32>,
    pub max_memory_mb: Option<u32>,
    pub jvm_args: Option<String>,
    pub created_at: String,
    pub last_played: Option<String>,
}

pub fn instances_root() -> AppResult<PathBuf> {
    let config = read_config()?;
    let data_dir = resolve_data_dir(&config)?;
    let dir = data_dir.join("instances");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn shared_mc_root() -> AppResult<PathBuf> {
    let config = read_config()?;
    let data_dir = resolve_data_dir(&config)?;
    let dir = data_dir.join("shared");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn instance_dir(id: &str) -> AppResult<PathBuf> {
    Ok(instances_root()?.join(id))
}

pub fn instance_mc_dir(id: &str) -> AppResult<PathBuf> {
    let dir = instance_dir(id)?.join(".minecraft");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn load_instance(id: &str) -> AppResult<GameInstance> {
    let meta_path = instance_dir(id)?.join("instance.json");
    if !meta_path.exists() {
        return Err(AppError::Message(format!("Instance '{id}' not found")));
    }
    let content = fs::read_to_string(meta_path)?;
    Ok(serde_json::from_str(&content)?)
}

pub fn save_instance(instance: &GameInstance) -> AppResult<()> {
    let dir = instance_dir(&instance.id)?;
    fs::create_dir_all(&dir)?;
    fs::create_dir_all(dir.join(".minecraft").join("mods"))?;
    let meta_path = dir.join("instance.json");
    fs::write(meta_path, serde_json::to_string_pretty(instance)?)?;
    Ok(())
}

#[tauri::command]
pub fn list_instances() -> AppResult<Vec<GameInstance>> {
    let root = instances_root()?;
    let mut instances = Vec::new();
    for entry in fs::read_dir(root)?.flatten() {
        let meta = entry.path().join("instance.json");
        if meta.exists() {
            if let Ok(text) = fs::read_to_string(&meta) {
                if let Ok(inst) = serde_json::from_str::<GameInstance>(&text) {
                    instances.push(inst);
                }
            }
        }
    }
    instances.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(instances)
}

#[tauri::command]
pub fn create_instance(
    name: String,
    mc_version: String,
    loader: LoaderType,
    loader_version: Option<String>,
) -> AppResult<GameInstance> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::Message("Instance name cannot be empty".to_string()));
    }
    let id = Uuid::new_v4().to_string();
    let instance = GameInstance {
        id: id.clone(),
        name: trimmed.to_string(),
        mc_version,
        loader,
        loader_version,
        java_path: None,
        min_memory_mb: None,
        max_memory_mb: None,
        jvm_args: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        last_played: None,
    };
    save_instance(&instance)?;

    // Set as active instance if none selected
    if let Ok(mut cfg) = read_config() {
        if cfg.active_instance_id.is_none() {
            cfg.active_instance_id = Some(id);
            let _ = write_config(&cfg);
        }
    }

    Ok(instance)
}

#[tauri::command]
pub fn update_instance(instance: GameInstance) -> AppResult<GameInstance> {
    save_instance(&instance)?;
    Ok(instance)
}

#[tauri::command]
pub fn delete_instance(id: String) -> AppResult<()> {
    let dir = instance_dir(&id)?;
    if dir.exists() {
        fs::remove_dir_all(dir)?;
    }
    if let Ok(mut cfg) = read_config() {
        if cfg.active_instance_id.as_deref() == Some(id.as_str()) {
            cfg.active_instance_id = None;
            let _ = write_config(&cfg);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn open_instance_folder(id: String) -> AppResult<()> {
    let dir = instance_mc_dir(&id)?;
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer").arg(dir).spawn()?;
    }
    Ok(())
}
