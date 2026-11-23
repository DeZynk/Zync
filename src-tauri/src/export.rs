use crate::config::{read_config, resolve_data_dir};
use crate::error::AppResult;
use crate::instances::{instance_mc_dir, load_instance, LoaderType};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha1::Sha1;
use sha2::{Digest, Sha512};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptions {
    pub instance_id: String,
    pub package_all_files: bool,
    pub keep_settings: bool,
    pub include_keybinds_and_volume: bool,
    pub include_multiplayer_servers: bool,
    pub include_jei_data: bool,
    pub include_mod_configs: bool,
    pub include_resource_packs: bool,
    pub include_shader_packs: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportScanSummary {
    pub total_mods: usize,
    pub modrinth_matched: usize,
    pub curseforge_matched: usize,
    pub unlisted_mods: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub output_path: String,
    pub total_mods: usize,
    pub modrinth_matched: usize,
    pub curseforge_matched: usize,
    pub bundled_directly: usize,
    pub unlisted_mods: Vec<String>,
}

#[derive(Debug, Clone)]
enum ModOrigin {
    Modrinth { download_url: String },
    CurseForge { download_url: String },
    Unlisted,
}

#[derive(Debug, Clone)]
struct ScannedMod {
    filename: String,
    path: PathBuf,
    size: u64,
    sha1: String,
    sha512: String,
    cf_fingerprint: u32,
    origin: ModOrigin,
}

/// Computes CurseForge's MurmurHash2 (seed = 1), skipping whitespace bytes (0x09, 0x0A, 0x0D, 0x20).
pub fn curseforge_murmur2(data: &[u8]) -> u32 {
    let filtered: Vec<u8> = data
        .iter()
        .copied()
        .filter(|&b| b != 9 && b != 10 && b != 13 && b != 32)
        .collect();

    const M: u32 = 0x5bd1e995;
    const R: u32 = 24;
    let len = filtered.len() as u32;
    let mut h: u32 = 1 ^ len;

    let mut chunks = filtered.chunks_exact(4);
    for chunk in &mut chunks {
        let mut k = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        k = k.wrapping_mul(M);
        k ^= k >> R;
        k = k.wrapping_mul(M);

        h = h.wrapping_mul(M);
        h ^= k;
    }

    let rem = chunks.remainder();
    match rem.len() {
        3 => {
            h ^= (rem[2] as u32) << 16;
            h ^= (rem[1] as u32) << 8;
            h ^= rem[0] as u32;
            h = h.wrapping_mul(M);
        }
        2 => {
            h ^= (rem[1] as u32) << 8;
            h ^= rem[0] as u32;
            h = h.wrapping_mul(M);
        }
        1 => {
            h ^= rem[0] as u32;
            h = h.wrapping_mul(M);
        }
        _ => {}
    }

    h ^= h >> 13;
    h = h.wrapping_mul(M);
    h ^= h >> 15;
    h
}

/// Filters `options.txt` to retain only keybinds (`key_*`) and sound/volume settings (`soundCategory_*`).
pub fn filter_keybinds_and_volume(options_txt: &str) -> String {
    options_txt
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("key_") || trimmed.starts_with("soundCategory_")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn sanitize_filename(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = sanitized.trim_matches('_');
    if trimmed.is_empty() {
        "zync-export".to_string()
    } else {
        trimmed.to_string()
    }
}

async fn scan_and_resolve_mods(mc_dir: &Path) -> AppResult<Vec<ScannedMod>> {
    let mods_dir = mc_dir.join("mods");
    if !mods_dir.exists() {
        return Ok(Vec::new());
    }

    let mut mods: Vec<ScannedMod> = Vec::new();
    for entry in fs::read_dir(&mods_dir)?.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let filename = entry.file_name().to_string_lossy().to_string();
        if !filename.ends_with(".jar") {
            continue;
        }
        let bytes = fs::read(&path)?;
        let size = bytes.len() as u64;

        let mut h1 = Sha1::new();
        h1.update(&bytes);
        let sha1 = hex::encode(h1.finalize());

        let mut h512 = Sha512::new();
        h512.update(&bytes);
        let sha512 = hex::encode(h512.finalize());

        let cf_fingerprint = curseforge_murmur2(&bytes);

        mods.push(ScannedMod {
            filename,
            path,
            size,
            sha1,
            sha512,
            cf_fingerprint,
            origin: ModOrigin::Unlisted,
        });
    }

    if mods.is_empty() {
        return Ok(mods);
    }

    let client = reqwest::Client::builder()
        .user_agent("ZyncLauncher/0.1.0 (github.com/Jamielin53/Zync)")
        .build()?;

    // 1. Batch lookup on Modrinth via SHA512
    let sha512_list: Vec<String> = mods.iter().map(|m| m.sha512.clone()).collect();
    if let Ok(resp) = client
        .post("https://api.modrinth.com/v2/version_files")
        .json(&json!({
            "hashes": sha512_list,
            "algorithm": "sha512"
        }))
        .send()
        .await
    {
        if resp.status().is_success() {
            if let Ok(map) = resp.json::<HashMap<String, Value>>().await {
                for m in &mut mods {
                    if let Some(version_obj) = map.get(&m.sha512) {
                        if let Some(files) = version_obj["files"].as_array() {
                            let matched_url = files
                                .iter()
                                .find(|f| f["hashes"]["sha512"].as_str() == Some(m.sha512.as_str()))
                                .or_else(|| files.first())
                                .and_then(|f| f["url"].as_str())
                                .map(String::from);
                            if let Some(url) = matched_url {
                                m.origin = ModOrigin::Modrinth { download_url: url };
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Batch lookup remaining unmatched mods on CurseForge via Murmur2 fingerprint
    let unmatched_fps: Vec<u32> = mods
        .iter()
        .filter(|m| matches!(m.origin, ModOrigin::Unlisted))
        .map(|m| m.cf_fingerprint)
        .collect();

    if !unmatched_fps.is_empty() {
        if let Ok(resp) = client
            .post("https://api.curse.tools/v1/cf/fingerprints/432")
            .json(&json!({ "fingerprints": unmatched_fps }))
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(body) = resp.json::<Value>().await {
                    if let Some(matches) = body["data"]["exactMatches"].as_array() {
                        let mut fp_to_url: HashMap<u32, String> = HashMap::new();
                        for entry in matches {
                            let file = &entry["file"];
                            if let Some(fp) = file["fileFingerprint"].as_u64() {
                                let url = file["downloadUrl"]
                                    .as_str()
                                    .filter(|s| !s.is_empty())
                                    .map(String::from)
                                    .or_else(|| {
                                        let id = file["id"].as_u64()?;
                                        let fname = file["fileName"].as_str()?;
                                        Some(format!(
                                            "https://edge.forgecdn.net/files/{}/{}/{}",
                                            id / 1000,
                                            id % 1000,
                                            fname
                                        ))
                                    });
                                if let Some(u) = url {
                                    fp_to_url.insert(fp as u32, u);
                                }
                            }
                        }
                        for m in &mut mods {
                            if matches!(m.origin, ModOrigin::Unlisted) {
                                if let Some(url) = fp_to_url.get(&m.cf_fingerprint) {
                                    m.origin = ModOrigin::CurseForge {
                                        download_url: url.clone(),
                                    };
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(mods)
}

fn write_zip_bytes(
    zip: &mut ZipWriter<fs::File>,
    written: &mut HashSet<String>,
    archive_path: &str,
    data: &[u8],
) -> AppResult<()> {
    let normalized = archive_path.replace('\\', "/");
    if !written.insert(normalized.clone()) {
        return Ok(());
    }
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file(normalized, opts)?;
    zip.write_all(data)?;
    Ok(())
}

fn add_file_to_zip(
    zip: &mut ZipWriter<fs::File>,
    written: &mut HashSet<String>,
    src_file: &Path,
    archive_path: &str,
) -> AppResult<()> {
    if !src_file.exists() || !src_file.is_file() {
        return Ok(());
    }
    let data = fs::read(src_file)?;
    write_zip_bytes(zip, written, archive_path, &data)
}

fn add_dir_to_zip(
    zip: &mut ZipWriter<fs::File>,
    written: &mut HashSet<String>,
    src_dir: &Path,
    archive_prefix: &str,
    skip_top_dirs: &[&str],
) -> AppResult<()> {
    if !src_dir.exists() || !src_dir.is_dir() {
        return Ok(());
    }

    let mut stack: Vec<(PathBuf, String, bool)> = vec![(src_dir.to_path_buf(), archive_prefix.to_string(), true)];
    while let Some((dir, prefix, is_root)) = stack.pop() {
        for entry in fs::read_dir(&dir)?.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if is_root && skip_top_dirs.iter().any(|&s| s.eq_ignore_ascii_case(&name)) {
                continue;
            }
            if name.ends_with(".lock") {
                continue;
            }
            let next_prefix = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            if path.is_dir() {
                stack.push((path, next_prefix, false));
            } else if path.is_file() {
                add_file_to_zip(zip, written, &path, &next_prefix)?;
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn scan_instance_mods_for_export(instance_id: String) -> AppResult<ExportScanSummary> {
    let mc_dir = instance_mc_dir(&instance_id)?;
    let mods = scan_and_resolve_mods(&mc_dir).await?;

    let mut modrinth_matched = 0;
    let mut curseforge_matched = 0;
    let mut unlisted_mods = Vec::new();

    for m in &mods {
        match m.origin {
            ModOrigin::Modrinth { .. } => modrinth_matched += 1,
            ModOrigin::CurseForge { .. } => curseforge_matched += 1,
            ModOrigin::Unlisted => unlisted_mods.push(m.filename.clone()),
        }
    }

    Ok(ExportScanSummary {
        total_mods: mods.len(),
        modrinth_matched,
        curseforge_matched,
        unlisted_mods,
    })
}

#[tauri::command]
pub async fn export_instance(options: ExportOptions) -> AppResult<ExportResult> {
    let config = read_config()?;
    let data_dir = resolve_data_dir(&config)?;
    let instance = load_instance(&options.instance_id)?;
    let mc_dir = instance_mc_dir(&options.instance_id)?;

    // Always verify which mods exist on Modrinth / CurseForge first
    let scanned_mods = scan_and_resolve_mods(&mc_dir).await?;

    let mut modrinth_matched = 0;
    let mut curseforge_matched = 0;
    let mut unlisted_mods = Vec::new();
    let mut bundled_directly = 0;

    for m in &scanned_mods {
        match m.origin {
            ModOrigin::Modrinth { .. } => modrinth_matched += 1,
            ModOrigin::CurseForge { .. } => curseforge_matched += 1,
            ModOrigin::Unlisted => unlisted_mods.push(m.filename.clone()),
        }
    }

    let exports_dir = data_dir.join("exports");
    fs::create_dir_all(&exports_dir)?;
    let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let archive_name = format!("{}-{timestamp}.mrpack", sanitize_filename(&instance.name));
    let output_path = exports_dir.join(archive_name);

    let file = fs::File::create(&output_path)?;
    let mut zip = ZipWriter::new(file);
    let mut written: HashSet<String> = HashSet::new();

    // Build modrinth.index.json dependencies
    let mut dependencies = serde_json::Map::new();
    dependencies.insert("minecraft".to_string(), Value::String(instance.mc_version.clone()));
    if let Some(lv) = &instance.loader_version {
        match instance.loader {
            LoaderType::Fabric => {
                dependencies.insert("fabric-loader".to_string(), Value::String(lv.clone()));
            }
            LoaderType::Quilt => {
                dependencies.insert("quilt-loader".to_string(), Value::String(lv.clone()));
            }
            LoaderType::Forge => {
                dependencies.insert("forge".to_string(), Value::String(lv.clone()));
            }
            LoaderType::NeoForge => {
                dependencies.insert("neoforge".to_string(), Value::String(lv.clone()));
            }
            LoaderType::Vanilla => {}
        }
    }

    let mut index_files: Vec<Value> = Vec::new();

    if options.package_all_files {
        // Package all instance gameplay files directly inside overrides/ (skipping engine & transient folders)
        add_dir_to_zip(
            &mut zip,
            &mut written,
            &mc_dir,
            "overrides",
            &[
                "versions",
                "libraries",
                "assets",
                "natives",
                "tmp",
                "logs",
                "crash-reports",
                "debug",
            ],
        )?;
        bundled_directly = scanned_mods.len();
    } else {
        // 1. Handle mods: Modrinth & CurseForge matched go to modrinth.index.json; unlisted ALWAYS bundled directly
        for m in &scanned_mods {
            match &m.origin {
                ModOrigin::Modrinth { download_url } | ModOrigin::CurseForge { download_url } => {
                    index_files.push(json!({
                        "path": format!("mods/{}", m.filename),
                        "hashes": {
                            "sha1": m.sha1,
                            "sha512": m.sha512
                        },
                        "env": {
                            "client": "required",
                            "server": "optional"
                        },
                        "downloads": [download_url],
                        "fileSize": m.size
                    }));
                }
                ModOrigin::Unlisted => {
                    add_file_to_zip(
                        &mut zip,
                        &mut written,
                        &m.path,
                        &format!("overrides/mods/{}", m.filename),
                    )?;
                    bundled_directly += 1;
                }
            }
        }

        // 2. Selective settings export
        if options.keep_settings {
            if options.include_keybinds_and_volume {
                let options_path = mc_dir.join("options.txt");
                if options_path.exists() {
                    if let Ok(raw) = fs::read_to_string(&options_path) {
                        let filtered = filter_keybinds_and_volume(&raw);
                        if !filtered.is_empty() {
                            write_zip_bytes(
                                &mut zip,
                                &mut written,
                                "overrides/options.txt",
                                filtered.as_bytes(),
                            )?;
                        }
                    }
                }
            }

            if options.include_multiplayer_servers {
                add_file_to_zip(
                    &mut zip,
                    &mut written,
                    &mc_dir.join("servers.dat"),
                    "overrides/servers.dat",
                )?;
                add_file_to_zip(
                    &mut zip,
                    &mut written,
                    &mc_dir.join("servers.dat_old"),
                    "overrides/servers.dat_old",
                )?;
            }

            if options.include_jei_data {
                add_dir_to_zip(
                    &mut zip,
                    &mut written,
                    &mc_dir.join("config").join("jei"),
                    "overrides/config/jei",
                    &[],
                )?;
            }

            if options.include_mod_configs {
                add_dir_to_zip(
                    &mut zip,
                    &mut written,
                    &mc_dir.join("config"),
                    "overrides/config",
                    &[],
                )?;
                add_dir_to_zip(
                    &mut zip,
                    &mut written,
                    &mc_dir.join("defaultconfigs"),
                    "overrides/defaultconfigs",
                    &[],
                )?;
            }

            if options.include_resource_packs {
                add_dir_to_zip(
                    &mut zip,
                    &mut written,
                    &mc_dir.join("resourcepacks"),
                    "overrides/resourcepacks",
                    &[],
                )?;
            }

            if options.include_shader_packs {
                add_dir_to_zip(
                    &mut zip,
                    &mut written,
                    &mc_dir.join("shaderpacks"),
                    "overrides/shaderpacks",
                    &[],
                )?;
            }
        }
    }

    let index_json = json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": timestamp.to_string(),
        "name": instance.name,
        "summary": "Exported from Zync Launcher",
        "files": index_files,
        "dependencies": Value::Object(dependencies)
    });

    let index_bytes = serde_json::to_vec_pretty(&index_json)?;
    write_zip_bytes(&mut zip, &mut written, "modrinth.index.json", &index_bytes)?;
    zip.finish()?;

    // Open Windows Explorer highlighting the exported file
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer")
            .arg(format!("/select,{}", output_path.display()))
            .spawn();
    }

    Ok(ExportResult {
        output_path: output_path.to_string_lossy().to_string(),
        total_mods: scanned_mods.len(),
        modrinth_matched,
        curseforge_matched,
        bundled_directly,
        unlisted_mods,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_keybinds_and_volume() {
        let sample = "\
version:3955
fov:0.0
renderDistance:12
soundCategory_master:0.65
soundCategory_music:0.2
key_key.attack:key.mouse.left
key_key.jump:key.keyboard.space
graphicsMode:1
";
        let out = filter_keybinds_and_volume(sample);
        assert!(out.contains("soundCategory_master:0.65"));
        assert!(out.contains("soundCategory_music:0.2"));
        assert!(out.contains("key_key.attack:key.mouse.left"));
        assert!(out.contains("key_key.jump:key.keyboard.space"));
        assert!(!out.contains("renderDistance"));
        assert!(!out.contains("graphicsMode"));
    }

    #[test]
    fn test_curseforge_murmur2_whitespace_ignored() {
        let a = curseforge_murmur2(b"hello world");
        let b = curseforge_murmur2(b"hello\t\r\n world");
        assert_eq!(a, b);
    }
}
