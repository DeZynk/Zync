use crate::config::{read_config, write_config, AccountType};
use crate::error::{AppError, AppResult};
use crate::instances::{instance_mc_dir, load_instance, save_instance, LoaderType};
use crate::java::{inspect_java_executable, scan_java_runtimes, DownloadProgress};
use crate::loaders::{ensure_forge_or_neoforge_installed, fetch_fabric_or_quilt_profile};
use futures_util::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter};
use tokio::sync::Semaphore;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

static RUNNING_PIDS: OnceLock<Mutex<Vec<u32>>> = OnceLock::new();

fn running_pids() -> &'static Mutex<Vec<u32>> {
    RUNNING_PIDS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Prefer `javaw.exe` for launching the game so Windows / NVIDIA / AMD GPU drivers
/// recognize it as a high-performance GUI application and route it to the discrete GPU.
fn prefer_javaw(java_path: &str) -> String {
    let p = Path::new(java_path);
    if let Some(parent) = p.parent() {
        let javaw = parent.join("javaw.exe");
        if javaw.exists() {
            return javaw.to_string_lossy().to_string();
        }
    }
    java_path.to_string()
}

#[tauri::command]
pub fn kill_instance(app: AppHandle) -> AppResult<()> {
    let pids: Vec<u32> = {
        let mut lock = running_pids()
            .lock()
            .map_err(|_| AppError::Message("Failed to lock running PIDs".to_string()))?;
        lock.drain(..).collect()
    };

    if pids.is_empty() {
        return Err(AppError::Message(
            "No running game instance to kill".to_string(),
        ));
    }

    for pid in pids {
        #[cfg(target_os = "windows")]
        {
            let _ = Command::new("taskkill")
                .args(["/F", "/T", "/PID", &pid.to_string()])
                .creation_flags(CREATE_NO_WINDOW)
                .output();
        }
    }

    let _ = app.emit(
        "game-log",
        "[Zync] Game instance terminated by user.".to_string(),
    );
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McVersionSummary {
    pub id: String,
    pub version_type: String,
    pub release_time: String,
    pub url: String,
}

#[derive(Deserialize)]
struct VersionManifestV2 {
    versions: Vec<ManifestVersionRaw>,
}

#[derive(Deserialize)]
struct ManifestVersionRaw {
    id: String,
    #[serde(rename = "type")]
    version_type: String,
    url: String,
    #[serde(rename = "releaseTime")]
    release_time: String,
}

#[derive(Clone)]
struct DownloadTask {
    url: String,
    dest: PathBuf,
    sha1: Option<String>,
}

fn sha1_file(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let mut hasher = Sha1::new();
    hasher.update(&bytes);
    Some(hex::encode(hasher.finalize()))
}

fn rule_allows_windows(rules: Option<&Vec<Value>>) -> bool {
    let Some(rules) = rules else {
        return true;
    };
    let mut allowed = false;
    for rule in rules {
        let action = rule["action"].as_str().unwrap_or("allow");
        if rule.get("features").is_some() {
            continue;
        }
        let os_match = if let Some(os) = rule.get("os") {
            let name_ok = os
                .get("name")
                .and_then(|v| v.as_str())
                .map(|n| n == "windows")
                .unwrap_or(true);
            let arch_ok = os
                .get("arch")
                .and_then(|v| v.as_str())
                .map(|a| a == "x86_64" || a == "amd64" || a == "x64")
                .unwrap_or(true);
            name_ok && arch_ok
        } else {
            true
        };
        if os_match {
            allowed = action == "allow";
        }
    }
    allowed
}

/// Extracts `groupId:artifactId(:classifier)` key for Maven library deduplication.
fn maven_artifact_key(name: &str) -> Option<String> {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    if parts.len() >= 4 {
        let classifier = parts[3].split('@').next().unwrap_or(parts[3]);
        Some(format!("{}:{}:{}", parts[0], parts[1], classifier))
    } else {
        Some(format!("{}:{}", parts[0], parts[1]))
    }
}

fn maven_name_to_path(name: &str) -> Option<String> {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let (version, ext) = if let Some((v, e)) = parts[2].split_once('@') {
        (v, e)
    } else {
        (parts[2], "jar")
    };
    let classifier = if parts.len() >= 4 {
        let c = parts[3].split('@').next().unwrap_or(parts[3]);
        format!("-{c}")
    } else {
        String::new()
    };
    let final_ext = if parts.len() >= 4 && parts[3].contains('@') {
        parts[3].split('@').nth(1).unwrap_or(ext)
    } else {
        ext
    };
    Some(format!(
        "{group}/{artifact}/{version}/{artifact}-{version}{classifier}.{final_ext}"
    ))
}

fn collect_arg_entries(arr: Option<&Vec<Value>>, out: &mut Vec<String>) {
    let Some(items) = arr else {
        return;
    };
    for item in items {
        if let Some(s) = item.as_str() {
            out.push(s.to_string());
        } else if let Some(obj) = item.as_object() {
            if !rule_allows_windows(obj.get("rules").and_then(|r| r.as_array())) {
                continue;
            }
            if let Some(val) = obj.get("value") {
                if let Some(s) = val.as_str() {
                    out.push(s.to_string());
                } else if let Some(list) = val.as_array() {
                    for v in list {
                        if let Some(s) = v.as_str() {
                            out.push(s.to_string());
                        }
                    }
                }
            }
        }
    }
}

fn infer_required_java_major(mc_version: &str, vanilla_json: &Value) -> u32 {
    if let Some(m) = vanilla_json["javaVersion"]["majorVersion"].as_u64() {
        return m as u32;
    }
    if let Some(rest) = mc_version.strip_prefix("1.") {
        let minor = rest
            .split('.')
            .next()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(21);
        let patch = rest
            .split('.')
            .nth(1)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        if minor <= 16 {
            8
        } else if minor < 20 || (minor == 20 && patch <= 4) {
            17
        } else {
            21
        }
    } else {
        21
    }
}

async fn run_downloads(
    app: &AppHandle,
    client: &reqwest::Client,
    tasks: Vec<DownloadTask>,
    concurrency: usize,
    label: &str,
) -> AppResult<()> {
    let pending: Vec<DownloadTask> = tasks
        .into_iter()
        .filter(|t| {
            if !t.dest.exists() {
                return true;
            }
            if let Some(expected) = &t.sha1 {
                sha1_file(&t.dest).as_deref() != Some(expected.as_str())
            } else {
                false
            }
        })
        .collect();

    let total = pending.len() as u64;
    if total == 0 {
        return Ok(());
    }

    let completed = Arc::new(AtomicU64::new(0));
    let sem = Arc::new(Semaphore::new(concurrency.max(1)));
    let label = label.to_string();

    let results: Vec<AppResult<()>> = stream::iter(pending)
        .map(|task| {
            let client = client.clone();
            let sem = sem.clone();
            let completed = completed.clone();
            let app = app.clone();
            let label = label.clone();

            async move {
                let _permit = sem
                    .acquire()
                    .await
                    .map_err(|e| AppError::Message(e.to_string()))?;
                if let Some(parent) = task.dest.parent() {
                    fs::create_dir_all(parent)?;
                }
                let bytes = client
                    .get(&task.url)
                    .send()
                    .await?
                    .error_for_status()?
                    .bytes()
                    .await?;

                if let Some(expected) = &task.sha1 {
                    let mut hasher = Sha1::new();
                    hasher.update(&bytes);
                    let actual = hex::encode(hasher.finalize());
                    if &actual != expected {
                        return Err(AppError::Message(format!(
                            "SHA1 mismatch for {}: expected {expected}, got {actual}",
                            task.dest.display()
                        )));
                    }
                }

                fs::write(&task.dest, &bytes)?;
                let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
                if done % 10 == 0 || done == total {
                    let _ = app.emit(
                        "download-progress",
                        DownloadProgress {
                            task: "minecraft".to_string(),
                            current: done,
                            total,
                            message: format!("{label} ({done}/{total})"),
                        },
                    );
                }
                Ok(())
            }
        })
        .buffer_unordered(concurrency.max(1))
        .collect()
        .await;

    for r in results {
        r?;
    }
    Ok(())
}

fn extract_natives_jar(jar_path: &Path, natives_dir: &Path) -> AppResult<()> {
    if !jar_path.exists() {
        return Ok(());
    }
    fs::create_dir_all(natives_dir)?;
    let file = fs::File::open(jar_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();
        if entry.is_dir() || name.starts_with("META-INF/") {
            continue;
        }
        if name.ends_with(".dll") {
            let file_name = Path::new(&name)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("native.dll");
            let out_path = natives_dir.join(file_name);
            let mut out = fs::File::create(out_path)?;
            std::io::copy(&mut entry, &mut out)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn list_minecraft_versions(include_snapshots: bool) -> AppResult<Vec<McVersionSummary>> {
    let client = reqwest::Client::builder()
        .user_agent("ZyncLauncher/0.1.0")
        .build()?;
    let manifest: VersionManifestV2 = client
        .get("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(manifest
        .versions
        .into_iter()
        .filter(|v| include_snapshots || v.version_type == "release")
        .map(|v| McVersionSummary {
            id: v.id,
            version_type: v.version_type,
            release_time: v.release_time,
            url: v.url,
        })
        .collect())
}

#[tauri::command]
pub async fn launch_instance(app: AppHandle, instance_id: String) -> AppResult<()> {
    let config = read_config()?;
    let mut instance = load_instance(&instance_id)?;

    // Full instance isolation: every instance owns its own versions/, libraries/, assets/, natives/
    let game_dir = instance_mc_dir(&instance_id)?;

    let client = reqwest::Client::builder()
        .user_agent("ZyncLauncher/0.1.0")
        .build()?;

    let _ = app.emit(
        "download-progress",
        DownloadProgress {
            task: "minecraft".to_string(),
            current: 0,
            total: 100,
            message: format!("Preparing Minecraft {}...", instance.mc_version),
        },
    );

    // 1. Fetch or load vanilla version.json inside this instance's .minecraft/versions/
    let vanilla_dir = game_dir.join("versions").join(&instance.mc_version);
    fs::create_dir_all(&vanilla_dir)?;
    let vanilla_json_path = vanilla_dir.join(format!("{}.json", instance.mc_version));

    let vanilla_json: Value = if vanilla_json_path.exists() {
        serde_json::from_str(&fs::read_to_string(&vanilla_json_path)?)?
    } else {
        let versions = list_minecraft_versions(true).await?;
        let target = versions
            .into_iter()
            .find(|v| v.id == instance.mc_version)
            .ok_or_else(|| {
                AppError::Message(format!("Minecraft version {} not found", instance.mc_version))
            })?;
        let json: Value = client
            .get(&target.url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        fs::write(&vanilla_json_path, serde_json::to_string_pretty(&json)?)?;
        json
    };

    // 2. Smart Auto-Match Java Runtime
    let required_major = infer_required_java_major(&instance.mc_version, &vanilla_json);
    let java_exe = if let Some(explicit) = instance.java_path.as_ref().filter(|s| !s.is_empty()) {
        explicit.clone()
    } else {
        let runtimes = tokio::task::spawn_blocking(scan_java_runtimes)
            .await
            .map_err(|e| AppError::Message(e.to_string()))??;

        if runtimes.is_empty() {
            return Err(AppError::Message(format!(
                "No Java runtime found. Minecraft {} requires Java {required_major}. Please install Java {required_major} in Download -> Java.",
                instance.mc_version
            )));
        }

        // Priority 1: Exact major version match (prefer Zync-managed runtime if multiple)
        let exact = runtimes
            .iter()
            .find(|r| r.major_version == required_major && r.is_managed)
            .or_else(|| runtimes.iter().find(|r| r.major_version == required_major));

        if let Some(rt) = exact {
            let _ = app.emit(
                "game-log",
                format!(
                    "[Zync] Auto-matched Java {} ({}) for Minecraft {}",
                    rt.major_version, rt.version, instance.mc_version
                ),
            );
            rt.path.clone()
        } else {
            if required_major == 8 && instance.loader == LoaderType::Forge {
                return Err(AppError::Message(format!(
                    "Minecraft {} with Forge strictly requires Java 8, which is not installed. Please go to Download -> Java and click 'Download Java 8'.",
                    instance.mc_version
                )));
            }

            let default_ok = config
                .default_java_path
                .as_ref()
                .and_then(|p| inspect_java_executable(Path::new(p), false))
                .filter(|rt| rt.major_version >= required_major);

            if let Some(rt) = default_ok {
                let _ = app.emit(
                    "game-log",
                    format!(
                        "[Zync] Using default Java {} ({}) for Minecraft {} (requires Java {})",
                        rt.major_version, rt.version, instance.mc_version, required_major
                    ),
                );
                rt.path
            } else if let Some(rt) = runtimes
                .iter()
                .filter(|r| r.major_version >= required_major)
                .min_by_key(|r| r.major_version)
            {
                let _ = app.emit(
                    "game-log",
                    format!(
                        "[Zync] Auto-selected compatible Java {} ({}) for Minecraft {} (requires Java {})",
                        rt.major_version, rt.version, instance.mc_version, required_major
                    ),
                );
                rt.path.clone()
            } else {
                return Err(AppError::Message(format!(
                    "Minecraft {} requires Java {required_major}, but only older Java versions were found. Please install Java {required_major} in Download -> Java.",
                    instance.mc_version
                )));
            }
        }
    };

    // 3. Collect downloads: client.jar + libraries (Loader first, then Vanilla) + natives + assets
    let mut download_tasks: Vec<DownloadTask> = Vec::new();
    let mut classpath_entries: Vec<PathBuf> = Vec::new();
    let mut native_jars: Vec<PathBuf> = Vec::new();
    let mut seen_artifacts: HashSet<String> = HashSet::new();
    let libraries_dir = game_dir.join("libraries");

    // Client JAR
    let client_jar = vanilla_dir.join(format!("{}.jar", instance.mc_version));
    if let Some(client_dl) = vanilla_json["downloads"]["client"].as_object() {
        if let Some(url) = client_dl.get("url").and_then(|v| v.as_str()) {
            download_tasks.push(DownloadTask {
                url: url.to_string(),
                dest: client_jar.clone(),
                sha1: client_dl
                    .get("sha1")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            });
        }
    }

    let mut collect_libs = |libs: &Vec<Value>, default_repo: &str| {
        for lib in libs {
            if !rule_allows_windows(lib["rules"].as_array()) {
                continue;
            }
            let lib_name = lib["name"].as_str().unwrap_or("");
            let artifact_key = maven_artifact_key(lib_name);
            let has_downloads_block = lib.get("downloads").is_some();
            let has_natives_block = lib.get("natives").is_some();

            // A. Standard artifact JAR (do NOT `continue;` on duplicate so classifiers below still run!)
            if let Some(artifact) = lib["downloads"]["artifact"].as_object() {
                if let (Some(url), Some(path)) = (
                    artifact.get("url").and_then(|v| v.as_str()),
                    artifact.get("path").and_then(|v| v.as_str()),
                ) {
                    let is_new = artifact_key
                        .as_ref()
                        .map(|k| seen_artifacts.insert(k.clone()))
                        .unwrap_or(true);
                    if is_new {
                        let dest = libraries_dir.join(path);
                        classpath_entries.push(dest.clone());
                        if !url.is_empty() {
                            download_tasks.push(DownloadTask {
                                url: url.to_string(),
                                dest,
                                sha1: artifact
                                    .get("sha1")
                                    .and_then(|v| v.as_str())
                                    .map(String::from),
                            });
                        }
                    }
                }
            } else if !has_downloads_block && !has_natives_block && !lib_name.is_empty() {
                // Maven-style entry without a Mojang `downloads` object (Fabric / Quilt / legacy Forge)
                let is_new = artifact_key
                    .as_ref()
                    .map(|k| seen_artifacts.insert(k.clone()))
                    .unwrap_or(true);
                if is_new {
                    if let Some(rel_path) = maven_name_to_path(lib_name) {
                        let base = lib["url"].as_str().unwrap_or(default_repo);
                        let base_trimmed = base.trim_end_matches('/');
                        let dest = libraries_dir.join(&rel_path);
                        classpath_entries.push(dest.clone());
                        download_tasks.push(DownloadTask {
                            url: format!("{base_trimmed}/{rel_path}"),
                            dest,
                            sha1: None,
                        });
                    }
                }
            }

            // B. Windows natives classifier (ALWAYS evaluated even if artifact was deduplicated!)
            if let Some(classifiers) = lib["downloads"]["classifiers"].as_object() {
                let win_native_key = lib["natives"]["windows"]
                    .as_str()
                    .map(|k| k.replace("${arch}", "64"));

                let target_key = if let Some(k) = win_native_key {
                    Some(k)
                } else if !has_natives_block && classifiers.contains_key("natives-windows") {
                    Some("natives-windows".to_string())
                } else {
                    None
                };

                if let Some(key) = target_key {
                    if let Some(native_obj) = classifiers.get(&key).and_then(|v| v.as_object()) {
                        if let (Some(url), Some(path)) = (
                            native_obj.get("url").and_then(|v| v.as_str()),
                            native_obj.get("path").and_then(|v| v.as_str()),
                        ) {
                            let dest = libraries_dir.join(path);
                            native_jars.push(dest.clone());
                            if !url.is_empty() {
                                download_tasks.push(DownloadTask {
                                    url: url.to_string(),
                                    dest,
                                    sha1: native_obj
                                        .get("sha1")
                                        .and_then(|v| v.as_str())
                                        .map(String::from),
                                });
                            }
                        }
                    }
                }
            }
        }
    };

    // 4. Resolve Mod Loader FIRST (so loader library versions override vanilla library versions)
    let mut main_class = vanilla_json["mainClass"]
        .as_str()
        .unwrap_or("net.minecraft.client.main.Main")
        .to_string();
    let mut extra_jvm_args: Vec<String> = Vec::new();
    let mut extra_game_args: Vec<String> = Vec::new();
    let mut legacy_mc_args: Option<String> = vanilla_json["minecraftArguments"]
        .as_str()
        .map(String::from);

    match instance.loader {
        LoaderType::Vanilla => {}
        LoaderType::Fabric | LoaderType::Quilt => {
            if let Some(lv) = &instance.loader_version {
                let profile =
                    fetch_fabric_or_quilt_profile(&instance.loader, &instance.mc_version, lv)
                        .await?;
                if let Some(mc) = profile["mainClass"].as_str() {
                    main_class = mc.to_string();
                }
                if let Some(legacy) = profile["minecraftArguments"].as_str() {
                    legacy_mc_args = Some(legacy.to_string());
                }
                if let Some(libs) = profile["libraries"].as_array() {
                    let default_repo = if instance.loader == LoaderType::Fabric {
                        "https://maven.fabricmc.net"
                    } else {
                        "https://maven.quiltmc.org/repository/release"
                    };
                    collect_libs(libs, default_repo);
                }
                collect_arg_entries(profile["arguments"]["jvm"].as_array(), &mut extra_jvm_args);
                collect_arg_entries(
                    profile["arguments"]["game"].as_array(),
                    &mut extra_game_args,
                );
            }
        }
        LoaderType::Forge | LoaderType::NeoForge => {
            if let Some(lv) = &instance.loader_version {
                let _ = app.emit(
                    "download-progress",
                    DownloadProgress {
                        task: "minecraft".to_string(),
                        current: 0,
                        total: 100,
                        message: format!("Installing {:?} {lv}...", instance.loader),
                    },
                );
                let loader_json_path = ensure_forge_or_neoforge_installed(
                    &game_dir,
                    &instance.loader,
                    &instance.mc_version,
                    lv,
                    &java_exe,
                )
                .await?;
                let loader_json: Value =
                    serde_json::from_str(&fs::read_to_string(&loader_json_path)?)?;
                if let Some(mc) = loader_json["mainClass"].as_str() {
                    main_class = mc.to_string();
                }
                if let Some(legacy) = loader_json["minecraftArguments"].as_str() {
                    legacy_mc_args = Some(legacy.to_string());
                }
                if let Some(libs) = loader_json["libraries"].as_array() {
                    collect_libs(libs, "https://maven.minecraftforge.net");
                }
                collect_arg_entries(
                    loader_json["arguments"]["jvm"].as_array(),
                    &mut extra_jvm_args,
                );
                collect_arg_entries(
                    loader_json["arguments"]["game"].as_array(),
                    &mut extra_game_args,
                );
            }
        }
    }

    // Collect Vanilla libraries AFTER loader libraries
    if let Some(libs) = vanilla_json["libraries"].as_array() {
        collect_libs(libs, "https://libraries.minecraft.net");
    }

    classpath_entries.push(client_jar);

    // 5. Asset index & objects inside this instance's .minecraft/assets/
    let assets_dir = game_dir.join("assets");
    let asset_index_id = vanilla_json["assetIndex"]["id"]
        .as_str()
        .unwrap_or(&instance.mc_version)
        .to_string();
    let asset_index_path = assets_dir
        .join("indexes")
        .join(format!("{asset_index_id}.json"));

    let asset_index_json: Value = if asset_index_path.exists() {
        serde_json::from_str(&fs::read_to_string(&asset_index_path)?)?
    } else {
        let url = vanilla_json["assetIndex"]["url"]
            .as_str()
            .ok_or_else(|| AppError::Message("Missing assetIndex URL".to_string()))?;
        let json: Value = client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        if let Some(parent) = asset_index_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&asset_index_path, serde_json::to_string(&json)?)?;
        json
    };

    if let Some(objects) = asset_index_json["objects"].as_object() {
        for (_name, obj) in objects {
            if let Some(hash) = obj["hash"].as_str() {
                let prefix = &hash[..2];
                let dest = assets_dir.join("objects").join(prefix).join(hash);
                download_tasks.push(DownloadTask {
                    url: format!("https://resources.download.minecraft.net/{prefix}/{hash}"),
                    dest,
                    sha1: Some(hash.to_string()),
                });
            }
        }
    }

    // 6. Execute downloads concurrently
    run_downloads(
        &app,
        &client,
        download_tasks,
        config.max_concurrent_downloads,
        "Downloading game files",
    )
    .await?;

    // 7. Extract natives into this instance's .minecraft/natives/<version>/
    let natives_dir = game_dir.join("natives").join(&instance.mc_version);
    for jar in &native_jars {
        extract_natives_jar(jar, &natives_dir)?;
    }

    let mut seen_cp = HashSet::new();
    let cp_str: String = classpath_entries
        .into_iter()
        .filter(|p| p.exists() && seen_cp.insert(p.clone()))
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(";");

    // 8. Resolve account
    let account = config
        .active_account_id
        .as_ref()
        .and_then(|id| config.accounts.iter().find(|a| &a.id == id));

    let (username, uuid, access_token, user_type) = match account {
        Some(acc) => (
            acc.username.clone(),
            acc.uuid.clone(),
            acc.access_token.clone().unwrap_or_else(|| "0".to_string()),
            if acc.account_type == AccountType::Microsoft {
                "msa"
            } else {
                "legacy"
            },
        ),
        None => (
            "Player".to_string(),
            "00000000000000000000000000000000".to_string(),
            "0".to_string(),
            "legacy",
        ),
    };

    // 9. Build JVM + Game arguments
    let min_mem = instance.min_memory_mb.unwrap_or(config.min_memory_mb);
    let max_mem = instance.max_memory_mb.unwrap_or(config.max_memory_mb);
    let jvm_flags = instance
        .jvm_args
        .clone()
        .unwrap_or_else(|| config.jvm_args.clone());

    let mut vars: HashMap<&str, String> = HashMap::new();
    vars.insert(
        "${natives_directory}",
        natives_dir.to_string_lossy().to_string(),
    );
    vars.insert("${launcher_name}", "Zync".to_string());
    vars.insert("${launcher_version}", "0.1.0".to_string());
    vars.insert("${classpath}", cp_str.clone());
    vars.insert(
        "${library_directory}",
        libraries_dir.to_string_lossy().to_string(),
    );
    vars.insert("${classpath_separator}", ";".to_string());
    vars.insert("${version_name}", instance.mc_version.clone());
    vars.insert("${version_type}", "release".to_string());
    vars.insert("${auth_player_name}", username.clone());
    vars.insert("${auth_uuid}", uuid.clone());
    vars.insert("${auth_access_token}", access_token.clone());
    vars.insert("${user_type}", user_type.to_string());
    vars.insert("${user_properties}", "{}".to_string());
    vars.insert("${game_directory}", game_dir.to_string_lossy().to_string());
    vars.insert("${assets_root}", assets_dir.to_string_lossy().to_string());
    vars.insert("${assets_index_name}", asset_index_id.clone());

    let substitute = |s: &str| -> String {
        let mut out = s.to_string();
        for (k, v) in &vars {
            out = out.replace(k, v);
        }
        out
    };

    let launch_binary = prefer_javaw(&java_exe);
    let mut cmd = Command::new(&launch_binary);
    cmd.current_dir(&game_dir);
    cmd.arg(format!("-Xms{min_mem}M"));
    cmd.arg(format!("-Xmx{max_mem}M"));
    for flag in jvm_flags.split_whitespace() {
        cmd.arg(flag);
    }
    cmd.arg(format!(
        "-Djava.library.path={}",
        natives_dir.to_string_lossy()
    ));
    cmd.arg("-Dminecraft.launcher.brand=Zync");
    cmd.arg("-Dminecraft.launcher.version=0.1.0");

    for arg in &extra_jvm_args {
        cmd.arg(substitute(arg));
    }

    cmd.arg("-cp");
    cmd.arg(&cp_str);
    cmd.arg(&main_class);

    if let Some(legacy_str) = legacy_mc_args {
        for token in legacy_str.split_whitespace() {
            cmd.arg(substitute(token));
        }
        cmd.args([
            "--width",
            &config.window_width.to_string(),
            "--height",
            &config.window_height.to_string(),
        ]);
    } else {
        cmd.args([
            "--username",
            &username,
            "--version",
            &instance.mc_version,
            "--gameDir",
            &game_dir.to_string_lossy(),
            "--assetsDir",
            &assets_dir.to_string_lossy(),
            "--assetIndex",
            &asset_index_id,
            "--uuid",
            &uuid,
            "--accessToken",
            &access_token,
            "--userType",
            user_type,
            "--width",
            &config.window_width.to_string(),
            "--height",
            &config.window_height.to_string(),
        ]);
    }

    for arg in &extra_game_args {
        cmd.arg(substitute(arg));
    }

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn()?;
    let pid = child.id();
    if let Ok(mut lock) = running_pids().lock() {
        lock.push(pid);
    }

    instance.last_played = Some(chrono::Utc::now().to_rfc3339());
    let _ = save_instance(&instance);

    let _ = app.emit(
        "download-progress",
        DownloadProgress {
            task: "minecraft".to_string(),
            current: 100,
            total: 100,
            message: "Game launched!".to_string(),
        },
    );

    if let Some(stdout) = child.stdout.take() {
        let app_clone = app.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                let _ = app_clone.emit("game-log", line);
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let app_clone = app.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                let _ = app_clone.emit("game-log", format!("[ERR] {line}"));
            }
        });
    }

    let app_exit = app.clone();
    std::thread::spawn(move || {
        let status = child.wait();
        if let Ok(mut lock) = running_pids().lock() {
            lock.retain(|&p| p != pid);
        }
        let code = status.map(|s| s.code().unwrap_or(0)).unwrap_or(-1);
        let _ = app_exit.emit("game-exited", code);
    });

    if config.close_on_launch {
        let mut cfg = read_config()?;
        cfg.active_instance_id = Some(instance.id);
        let _ = write_config(&cfg);
    }

    Ok(())
}
