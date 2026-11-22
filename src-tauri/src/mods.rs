use crate::error::{AppError, AppResult};
use crate::instances::{create_instance, instance_mc_dir, load_instance, GameInstance, LoaderType};
use crate::java::DownloadProgress;
use futures_util::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Semaphore;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ContentSource {
    Modrinth,
    Curseforge,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ContentType {
    Mod,
    Resourcepack,
    Shader,
    Modpack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentSearchResult {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub author: String,
    pub source: ContentSource,
    pub content_type: ContentType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledMod {
    pub filename: String,
    pub name: String,
    pub enabled: bool,
    pub size_bytes: u64,
}

#[derive(Deserialize)]
struct ModrinthSearchResponse {
    hits: Vec<ModrinthHit>,
}

#[derive(Deserialize)]
struct ModrinthHit {
    project_id: String,
    slug: String,
    title: String,
    description: String,
    icon_url: Option<String>,
    downloads: u64,
    author: String,
}

#[derive(Deserialize)]
struct ModrinthVersionRaw {
    game_versions: Vec<String>,
    files: Vec<ModrinthFileRaw>,
}

#[derive(Deserialize)]
struct ModrinthFileRaw {
    url: String,
    filename: String,
    primary: bool,
}

#[derive(Deserialize)]
struct CfSearchResp {
    data: Vec<CfModRaw>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfModRaw {
    id: u64,
    name: String,
    slug: String,
    summary: String,
    download_count: f64,
    logo: Option<CfLogoRaw>,
    authors: Vec<CfAuthorRaw>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfLogoRaw {
    thumbnail_url: Option<String>,
    url: Option<String>,
}

#[derive(Deserialize)]
struct CfAuthorRaw {
    name: String,
}

#[derive(Deserialize)]
struct CfFilesResp {
    data: Vec<CfFileRaw>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfFileRaw {
    id: u64,
    file_name: String,
    download_url: Option<String>,
}

fn loader_to_modrinth_str(loader: &LoaderType) -> Option<&'static str> {
    match loader {
        LoaderType::Fabric => Some("fabric"),
        LoaderType::Quilt => Some("quilt"),
        LoaderType::Forge => Some("forge"),
        LoaderType::NeoForge => Some("neoforge"),
        LoaderType::Vanilla => None,
    }
}

fn loader_to_cf_type(loader: &LoaderType) -> Option<&'static str> {
    match loader {
        LoaderType::Forge => Some("1"),
        LoaderType::Fabric => Some("4"),
        LoaderType::Quilt => Some("5"),
        LoaderType::NeoForge => Some("6"),
        LoaderType::Vanilla => None,
    }
}

fn content_type_to_modrinth_str(ct: &ContentType) -> &'static str {
    match ct {
        ContentType::Mod => "mod",
        ContentType::Resourcepack => "resourcepack",
        ContentType::Shader => "shader",
        ContentType::Modpack => "modpack",
    }
}

fn content_type_to_cf_class_id(ct: &ContentType) -> &'static str {
    match ct {
        ContentType::Mod => "6",
        ContentType::Resourcepack => "12",
        ContentType::Shader => "6552",
        ContentType::Modpack => "4471",
    }
}

fn cf_fallback_download_url(file_id: u64, filename: &str) -> String {
    let major = file_id / 1000;
    let minor = file_id % 1000;
    format!("https://edge.forgecdn.net/files/{major}/{minor}/{filename}")
}

#[tauri::command]
pub async fn search_content(
    source: ContentSource,
    content_type: ContentType,
    query: String,
    mc_version: Option<String>,
    loader: Option<LoaderType>,
    limit: Option<u32>,
) -> AppResult<Vec<ContentSearchResult>> {
    let client = reqwest::Client::builder()
        .user_agent("ZyncLauncher/0.1.0 (github.com/Jamielin53)")
        .build()?;
    let lim = limit.unwrap_or(24);

    match source {
        ContentSource::Modrinth => {
            let mut facets: Vec<Vec<String>> = vec![vec![format!(
                "project_type:{}",
                content_type_to_modrinth_str(&content_type)
            )]];
            if let Some(mc) = mc_version.filter(|s| !s.is_empty()) {
                facets.push(vec![format!("versions:{mc}")]);
            }
            if content_type == ContentType::Mod || content_type == ContentType::Modpack {
                if let Some(l) = loader.as_ref().and_then(loader_to_modrinth_str) {
                    facets.push(vec![format!("categories:{l}")]);
                }
            }

            let facets_json = serde_json::to_string(&facets)?;
            let resp: ModrinthSearchResponse = client
                .get("https://api.modrinth.com/v2/search")
                .query(&[
                    ("query", query.as_str()),
                    ("facets", facets_json.as_str()),
                    ("limit", &lim.to_string()),
                ])
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;

            Ok(resp
                .hits
                .into_iter()
                .map(|h| ContentSearchResult {
                    project_id: h.project_id,
                    slug: h.slug,
                    title: h.title,
                    description: h.description,
                    icon_url: h.icon_url,
                    downloads: h.downloads,
                    author: h.author,
                    source: ContentSource::Modrinth,
                    content_type: content_type.clone(),
                })
                .collect())
        }
        ContentSource::Curseforge => {
            let class_id = content_type_to_cf_class_id(&content_type);
            let mut params: Vec<(&str, String)> = vec![
                ("gameId", "432".to_string()),
                ("classId", class_id.to_string()),
                ("searchFilter", query),
                ("sortField", "2".to_string()),
                ("sortOrder", "desc".to_string()),
                ("pageSize", lim.to_string()),
            ];
            if let Some(mc) = mc_version.filter(|s| !s.is_empty()) {
                params.push(("gameVersion", mc));
            }
            if content_type == ContentType::Mod || content_type == ContentType::Modpack {
                if let Some(lt) = loader.as_ref().and_then(loader_to_cf_type) {
                    params.push(("modLoaderType", lt.to_string()));
                }
            }

            let resp: CfSearchResp = client
                .get("https://api.curse.tools/v1/cf/mods/search")
                .query(&params)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;

            Ok(resp
                .data
                .into_iter()
                .map(|m| ContentSearchResult {
                    project_id: m.id.to_string(),
                    slug: m.slug,
                    title: m.name,
                    description: m.summary,
                    icon_url: m.logo.and_then(|l| l.thumbnail_url.or(l.url)),
                    downloads: m.download_count as u64,
                    author: m
                        .authors
                        .into_iter()
                        .next()
                        .map(|a| a.name)
                        .unwrap_or_else(|| "Unknown".to_string()),
                    source: ContentSource::Curseforge,
                    content_type: content_type.clone(),
                })
                .collect())
        }
    }
}

#[tauri::command]
pub async fn install_content_item(
    source: ContentSource,
    content_type: ContentType,
    instance_id: String,
    project_id: String,
    mc_version: Option<String>,
    loader: Option<LoaderType>,
) -> AppResult<String> {
    let inst = load_instance(&instance_id)?;
    let target_subfolder = match content_type {
        ContentType::Mod => "mods",
        ContentType::Resourcepack => "resourcepacks",
        ContentType::Shader => "shaderpacks",
        ContentType::Modpack => {
            return Err(AppError::Message(
                "Use install_modpack for modpacks".to_string(),
            ))
        }
    };

    let target_dir = instance_mc_dir(&instance_id)?.join(target_subfolder);
    fs::create_dir_all(&target_dir)?;

    let client = reqwest::Client::builder()
        .user_agent("ZyncLauncher/0.1.0 (github.com/Jamielin53)")
        .build()?;

    let mc = mc_version.unwrap_or(inst.mc_version);
    let ld = loader.unwrap_or(inst.loader);

    let (download_url, filename) = match source {
        ContentSource::Modrinth => {
            let mut req = client.get(format!(
                "https://api.modrinth.com/v2/project/{project_id}/version"
            ));
            if !mc.is_empty() {
                let gv = serde_json::to_string(&vec![&mc])?;
                req = req.query(&[("game_versions", gv)]);
            }
            if content_type == ContentType::Mod {
                if let Some(l) = loader_to_modrinth_str(&ld) {
                    let lds = serde_json::to_string(&vec![l])?;
                    req = req.query(&[("loaders", lds)]);
                }
            }
            let mut versions: Vec<ModrinthVersionRaw> =
                req.send().await?.error_for_status()?.json().await?;

            // Fallback without strict loader/version filter if empty for resourcepacks/shaders
            if versions.is_empty() && content_type != ContentType::Mod {
                versions = client
                    .get(format!(
                        "https://api.modrinth.com/v2/project/{project_id}/version"
                    ))
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
            }

            let ver = versions.into_iter().next().ok_or_else(|| {
                AppError::Message(format!("No compatible file found for MC {mc}"))
            })?;
            let file = ver
                .files
                .iter()
                .find(|f| f.primary)
                .or_else(|| ver.files.first())
                .ok_or_else(|| AppError::Message("Version has no downloadable files".to_string()))?;
            (file.url.clone(), file.filename.clone())
        }
        ContentSource::Curseforge => {
            let mut params: Vec<(&str, String)> = vec![("pageSize", "20".to_string())];
            if !mc.is_empty() {
                params.push(("gameVersion", mc.clone()));
            }
            if content_type == ContentType::Mod {
                if let Some(lt) = loader_to_cf_type(&ld) {
                    params.push(("modLoaderType", lt.to_string()));
                }
            }

            let mut resp: CfFilesResp = client
                .get(format!("https://api.curse.tools/v1/cf/mods/{project_id}/files"))
                .query(&params)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;

            if resp.data.is_empty() && content_type != ContentType::Mod {
                resp = client
                    .get(format!("https://api.curse.tools/v1/cf/mods/{project_id}/files"))
                    .query(&[("pageSize", "10")])
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
            }

            let file = resp.data.into_iter().next().ok_or_else(|| {
                AppError::Message(format!("No compatible CurseForge file found for MC {mc}"))
            })?;
            let url = file
                .download_url
                .unwrap_or_else(|| cf_fallback_download_url(file.id, &file.file_name));
            (url, file.file_name)
        }
    };

    let safe_name = Path::new(&filename)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| AppError::Message("Invalid filename".to_string()))?
        .to_string();

    let bytes = client
        .get(&download_url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    fs::write(target_dir.join(&safe_name), &bytes)?;

    Ok(safe_name)
}

#[tauri::command]
pub async fn install_modpack(
    app: AppHandle,
    source: ContentSource,
    project_id: String,
    title: String,
) -> AppResult<GameInstance> {
    let client = reqwest::Client::builder()
        .user_agent("ZyncLauncher/0.1.0 (github.com/Jamielin53)")
        .build()?;

    let _ = app.emit(
        "download-progress",
        DownloadProgress {
            task: "modpack".to_string(),
            current: 0,
            total: 100,
            message: format!("Fetching modpack '{title}' metadata..."),
        },
    );

    match source {
        ContentSource::Modrinth => {
            let versions: Vec<ModrinthVersionRaw> = client
                .get(format!(
                    "https://api.modrinth.com/v2/project/{project_id}/version"
                ))
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let ver = versions
                .into_iter()
                .next()
                .ok_or_else(|| AppError::Message("No modpack version found".to_string()))?;
            let file = ver
                .files
                .iter()
                .find(|f| f.primary)
                .or_else(|| ver.files.first())
                .ok_or_else(|| AppError::Message("No .mrpack file found".to_string()))?;

            let _ = app.emit(
                "download-progress",
                DownloadProgress {
                    task: "modpack".to_string(),
                    current: 10,
                    total: 100,
                    message: format!("Downloading {}...", file.filename),
                },
            );

            let archive_bytes = client
                .get(&file.url)
                .send()
                .await?
                .error_for_status()?
                .bytes()
                .await?;

            let cursor = io::Cursor::new(archive_bytes);
            let mut zip = zip::ZipArchive::new(cursor)?;

            // Read modrinth.index.json
            let index_json: Value = {
                let mut index_file = zip.by_name("modrinth.index.json")?;
                let mut s = String::new();
                io::Read::read_to_string(&mut index_file, &mut s)?;
                serde_json::from_str(&s)?
            };

            let deps = &index_json["dependencies"];
            let mc_version = deps["minecraft"]
                .as_str()
                .or_else(|| ver.game_versions.first().map(|s| s.as_str()))
                .unwrap_or("1.21.1")
                .to_string();

            let (loader, loader_version) = if let Some(v) = deps["fabric-loader"].as_str() {
                (LoaderType::Fabric, Some(v.to_string()))
            } else if let Some(v) = deps["quilt-loader"].as_str() {
                (LoaderType::Quilt, Some(v.to_string()))
            } else if let Some(v) = deps["neoforge"].as_str() {
                (LoaderType::NeoForge, Some(v.to_string()))
            } else if let Some(v) = deps["forge"].as_str() {
                (LoaderType::Forge, Some(v.to_string()))
            } else {
                (LoaderType::Vanilla, None)
            };

            let instance = create_instance(title.clone(), mc_version, loader, loader_version)?;
            let mc_dir = instance_mc_dir(&instance.id)?;

            // Extract overrides/
            for i in 0..zip.len() {
                let mut entry = zip.by_index(i)?;
                let name = entry.name().to_string();
                let rel = name
                    .strip_prefix("overrides/")
                    .or_else(|| name.strip_prefix("client-overrides/"));
                if let Some(rel_path) = rel {
                    if rel_path.is_empty() {
                        continue;
                    }
                    let out_path = mc_dir.join(rel_path);
                    if entry.is_dir() {
                        fs::create_dir_all(&out_path)?;
                    } else {
                        if let Some(p) = out_path.parent() {
                            fs::create_dir_all(p)?;
                        }
                        let mut outfile = fs::File::create(&out_path)?;
                        io::copy(&mut entry, &mut outfile)?;
                    }
                }
            }

            // Download all files listed in modrinth.index.json
            let mut tasks: Vec<(String, std::path::PathBuf)> = Vec::new();
            if let Some(files) = index_json["files"].as_array() {
                for f in files {
                    if let (Some(path_str), Some(downloads)) =
                        (f["path"].as_str(), f["downloads"].as_array())
                    {
                        if let Some(url) = downloads.first().and_then(|u| u.as_str()) {
                            tasks.push((url.to_string(), mc_dir.join(path_str)));
                        }
                    }
                }
            }

            let total = tasks.len() as u64;
            let done = Arc::new(AtomicU64::new(0));
            let sem = Arc::new(Semaphore::new(12));

            let results: Vec<AppResult<()>> = stream::iter(tasks)
                .map(|(url, dest)| {
                    let client = client.clone();
                    let sem = sem.clone();
                    let done = done.clone();
                    let app = app.clone();
                    async move {
                        let _p = sem.acquire().await.map_err(|e| AppError::Message(e.to_string()))?;
                        if let Some(parent) = dest.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        let b = client.get(&url).send().await?.error_for_status()?.bytes().await?;
                        fs::write(&dest, &b)?;
                        let cur = done.fetch_add(1, Ordering::Relaxed) + 1;
                        let _ = app.emit(
                            "download-progress",
                            DownloadProgress {
                                task: "modpack".to_string(),
                                current: cur,
                                total,
                                message: format!("Downloading modpack files ({cur}/{total})..."),
                            },
                        );
                        Ok(())
                    }
                })
                .buffer_unordered(12)
                .collect()
                .await;

            for r in results {
                r?;
            }

            let _ = app.emit(
                "download-progress",
                DownloadProgress {
                    task: "modpack".to_string(),
                    current: 100,
                    total: 100,
                    message: format!("Modpack '{title}' installed!"),
                },
            );

            Ok(instance)
        }
        ContentSource::Curseforge => {
            let resp: CfFilesResp = client
                .get(format!("https://api.curse.tools/v1/cf/mods/{project_id}/files"))
                .query(&[("pageSize", "5")])
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let file = resp
                .data
                .into_iter()
                .next()
                .ok_or_else(|| AppError::Message("No CurseForge modpack file found".to_string()))?;
            let url = file
                .download_url
                .unwrap_or_else(|| cf_fallback_download_url(file.id, &file.file_name));

            let archive_bytes = client.get(&url).send().await?.error_for_status()?.bytes().await?;
            let cursor = io::Cursor::new(archive_bytes);
            let mut zip = zip::ZipArchive::new(cursor)?;

            let manifest: Value = {
                let mut mf = zip.by_name("manifest.json")?;
                let mut s = String::new();
                io::Read::read_to_string(&mut mf, &mut s)?;
                serde_json::from_str(&s)?
            };

            let mc_version = manifest["minecraft"]["version"]
                .as_str()
                .unwrap_or("1.20.1")
                .to_string();
            let loader_id = manifest["minecraft"]["modLoaders"]
                .as_array()
                .and_then(|a| a.first())
                .and_then(|l| l["id"].as_str())
                .unwrap_or("");

            let (loader, loader_version) = if let Some(v) = loader_id.strip_prefix("forge-") {
                (LoaderType::Forge, Some(v.to_string()))
            } else if let Some(v) = loader_id.strip_prefix("neoforge-") {
                (LoaderType::NeoForge, Some(v.to_string()))
            } else if let Some(v) = loader_id.strip_prefix("fabric-") {
                (LoaderType::Fabric, Some(v.to_string()))
            } else if let Some(v) = loader_id.strip_prefix("quilt-") {
                (LoaderType::Quilt, Some(v.to_string()))
            } else {
                (LoaderType::Vanilla, None)
            };

            let instance = create_instance(title.clone(), mc_version, loader, loader_version)?;
            let mc_dir = instance_mc_dir(&instance.id)?;
            let overrides_prefix = format!(
                "{}/",
                manifest["overrides"].as_str().unwrap_or("overrides")
            );

            for i in 0..zip.len() {
                let mut entry = zip.by_index(i)?;
                let name = entry.name().to_string();
                if let Some(rel_path) = name.strip_prefix(&overrides_prefix) {
                    if rel_path.is_empty() {
                        continue;
                    }
                    let out_path = mc_dir.join(rel_path);
                    if entry.is_dir() {
                        fs::create_dir_all(&out_path)?;
                    } else {
                        if let Some(p) = out_path.parent() {
                            fs::create_dir_all(p)?;
                        }
                        let mut outfile = fs::File::create(&out_path)?;
                        io::copy(&mut entry, &mut outfile)?;
                    }
                }
            }

            let _ = app.emit(
                "download-progress",
                DownloadProgress {
                    task: "modpack".to_string(),
                    current: 100,
                    total: 100,
                    message: format!("Modpack '{title}' installed!"),
                },
            );

            Ok(instance)
        }
    }
}

#[tauri::command]
pub fn list_installed_mods(instance_id: String) -> AppResult<Vec<InstalledMod>> {
    let mods_dir = instance_mc_dir(&instance_id)?.join("mods");
    if !mods_dir.exists() {
        return Ok(Vec::new());
    }

    let mut mods = Vec::new();
    for entry in fs::read_dir(mods_dir)?.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let filename = entry.file_name().to_string_lossy().to_string();
        let enabled = filename.ends_with(".jar");
        let disabled = filename.ends_with(".jar.disabled");
        if !enabled && !disabled {
            continue;
        }
        let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
        let display_name = filename
            .trim_end_matches(".disabled")
            .trim_end_matches(".jar")
            .to_string();
        mods.push(InstalledMod {
            filename,
            name: display_name,
            enabled,
            size_bytes,
        });
    }
    mods.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(mods)
}

#[tauri::command]
pub fn toggle_mod(instance_id: String, filename: String) -> AppResult<InstalledMod> {
    let mods_dir = instance_mc_dir(&instance_id)?.join("mods");
    let src = mods_dir.join(&filename);
    if !src.exists() {
        return Err(AppError::Message(format!("Mod file not found: {filename}")));
    }

    let (new_name, enabled) = if filename.ends_with(".jar.disabled") {
        (filename.trim_end_matches(".disabled").to_string(), true)
    } else if filename.ends_with(".jar") {
        (format!("{filename}.disabled"), false)
    } else {
        return Err(AppError::Message("Unsupported mod file extension".to_string()));
    };

    let dst = mods_dir.join(&new_name);
    fs::rename(&src, &dst)?;
    let size_bytes = fs::metadata(&dst).map(|m| m.len()).unwrap_or(0);

    Ok(InstalledMod {
        filename: new_name.clone(),
        name: new_name
            .trim_end_matches(".disabled")
            .trim_end_matches(".jar")
            .to_string(),
        enabled,
        size_bytes,
    })
}

#[tauri::command]
pub fn delete_mod(instance_id: String, filename: String) -> AppResult<()> {
    let path = instance_mc_dir(&instance_id)?.join("mods").join(&filename);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}
