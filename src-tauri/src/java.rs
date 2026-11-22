use crate::config::{read_config, resolve_data_dir};
use crate::error::{AppError, AppResult};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaRuntime {
    pub path: String,
    pub version: String,
    pub major_version: u32,
    pub vendor: String,
    pub is_managed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub task: String,
    pub current: u64,
    pub total: u64,
    pub message: String,
}

fn parse_major_version(version_str: &str) -> u32 {
    let trimmed = version_str.trim().trim_matches('"');
    if let Some(rest) = trimmed.strip_prefix("1.") {
        rest.split('.')
            .next()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(8)
    } else {
        trimmed
            .split(|c: char| !c.is_ascii_digit())
            .next()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0)
    }
}

pub fn inspect_java_executable(java_path: &Path, is_managed: bool) -> Option<JavaRuntime> {
    if !java_path.exists() {
        return None;
    }

    let mut cmd = std::process::Command::new(java_path);
    cmd.arg("-version");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd.output().ok()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = if stderr.trim().is_empty() { stdout } else { stderr };

    let first_line = combined.lines().next().unwrap_or("");
    let version = first_line
        .split('"')
        .nth(1)
        .unwrap_or("unknown")
        .to_string();
    let major_version = parse_major_version(&version);

    let vendor = if combined.contains("Temurin") || combined.contains("Adoptium") {
        "Eclipse Adoptium".to_string()
    } else if combined.contains("Microsoft") {
        "Microsoft".to_string()
    } else if combined.contains("Zulu") || combined.contains("Azul") {
        "Azul Zulu".to_string()
    } else if combined.contains("Corretto") {
        "Amazon Corretto".to_string()
    } else if combined.contains("Oracle") || combined.contains("Java(TM)") {
        "Oracle".to_string()
    } else {
        "OpenJDK".to_string()
    };

    Some(JavaRuntime {
        path: java_path.to_string_lossy().to_string(),
        version,
        major_version,
        vendor,
        is_managed,
    })
}

fn collect_java_bins_in_dir(dir: &Path, out: &mut Vec<(PathBuf, bool)>, is_managed: bool) {
    if !dir.exists() || !dir.is_dir() {
        return;
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let candidate = path.join("bin").join("java.exe");
                if candidate.exists() {
                    out.push((candidate, is_managed));
                } else {
                    // Check one level deeper (e.g. extracted adoptium jdk-21+35-jre folder)
                    if let Ok(sub_entries) = fs::read_dir(&path) {
                        for sub in sub_entries.flatten() {
                            let sub_candidate = sub.path().join("bin").join("java.exe");
                            if sub_candidate.exists() {
                                out.push((sub_candidate, is_managed));
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn scan_java_runtimes() -> AppResult<Vec<JavaRuntime>> {
    let config = read_config()?;
    let data_dir = resolve_data_dir(&config)?;
    let managed_dir = data_dir.join("runtimes");

    let mut candidates: Vec<(PathBuf, bool)> = Vec::new();

    // 1. Zync-managed runtimes
    collect_java_bins_in_dir(&managed_dir, &mut candidates, true);

    // 2. JAVA_HOME
    if let Ok(java_home) = std::env::var("JAVA_HOME") {
        candidates.push((PathBuf::from(java_home).join("bin").join("java.exe"), false));
    }

    // 3. PATH
    if let Ok(path_var) = std::env::var("PATH") {
        for segment in std::env::split_paths(&path_var) {
            let candidate = segment.join("java.exe");
            if candidate.exists() {
                candidates.push((candidate, false));
            }
        }
    }

    // 4. Common Windows install locations
    let common_roots = [
        r"C:\Program Files\Java",
        r"C:\Program Files\Eclipse Adoptium",
        r"C:\Program Files\AdoptOpenJDK",
        r"C:\Program Files\Microsoft",
        r"C:\Program Files\Zulu",
        r"C:\Program Files\BellSoft",
        r"C:\Program Files\Amazon Corretto",
        r"C:\Program Files (x86)\Java",
    ];
    for root in common_roots {
        collect_java_bins_in_dir(Path::new(root), &mut candidates, false);
    }

    let mut seen = HashSet::new();
    let mut runtimes = Vec::new();

    for (path, is_managed) in candidates {
        let canonical = fs::canonicalize(&path).unwrap_or(path);
        let key = canonical.to_string_lossy().to_lowercase();
        if seen.insert(key) {
            if let Some(rt) = inspect_java_executable(&canonical, is_managed) {
                runtimes.push(rt);
            }
        }
    }

    runtimes.sort_by(|a, b| b.major_version.cmp(&a.major_version));
    Ok(runtimes)
}

#[tauri::command]
pub async fn detect_java_runtimes() -> AppResult<Vec<JavaRuntime>> {
    tokio::task::spawn_blocking(scan_java_runtimes)
        .await
        .map_err(|e| AppError::Message(format!("Java scan task failed: {e}")))?
}

#[tauri::command]
pub async fn test_java_runtime(path: String) -> AppResult<JavaRuntime> {
    tokio::task::spawn_blocking(move || {
        inspect_java_executable(Path::new(&path), false)
            .ok_or_else(|| AppError::Message(format!("Invalid or non-executable Java binary: {path}")))
    })
    .await
    .map_err(|e| AppError::Message(format!("Java test task failed: {e}")))?
}

#[derive(Deserialize)]
struct AdoptiumAsset {
    binary: AdoptiumBinary,
}

#[derive(Deserialize)]
struct AdoptiumBinary {
    package: AdoptiumPackage,
}

#[derive(Deserialize)]
struct AdoptiumPackage {
    link: String,
    size: u64,
    name: String,
}

#[tauri::command]
pub async fn download_java_runtime(
    app: AppHandle,
    major_version: u32,
) -> AppResult<JavaRuntime> {
    let config = read_config()?;
    let data_dir = resolve_data_dir(&config)?;
    let runtimes_dir = data_dir.join("runtimes");
    fs::create_dir_all(&runtimes_dir)?;

    let url = format!(
        "https://api.adoptium.net/v3/assets/latest/{major_version}/hotspot?architecture=x64&image_type=jre&os=windows&vendor=eclipse"
    );

    let client = reqwest::Client::builder()
        .user_agent("ZyncLauncher/0.1.0")
        .build()?;

    let _ = app.emit(
        "download-progress",
        DownloadProgress {
            task: "java".to_string(),
            current: 0,
            total: 100,
            message: format!("Fetching Java {major_version} metadata..."),
        },
    );

    let assets: Vec<AdoptiumAsset> = client.get(&url).send().await?.error_for_status()?.json().await?;
    let asset = assets
        .into_iter()
        .next()
        .ok_or_else(|| AppError::Message(format!("No Adoptium JRE found for Java {major_version}")))?;

    let pkg = asset.binary.package;
    let total_size = pkg.size;
    let zip_path = runtimes_dir.join(&pkg.name);

    let resp = client.get(&pkg.link).send().await?.error_for_status()?;
    let mut stream = resp.bytes_stream();
    let mut file = fs::File::create(&zip_path)?;
    let mut downloaded: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        io::Write::write_all(&mut file, &chunk)?;
        downloaded += chunk.len() as u64;
        let _ = app.emit(
            "download-progress",
            DownloadProgress {
                task: "java".to_string(),
                current: downloaded,
                total: total_size,
                message: format!("Downloading Java {major_version} ({:.1} MB)...", downloaded as f64 / 1_048_576.0),
            },
        );
    }
    drop(file);

    let _ = app.emit(
        "download-progress",
        DownloadProgress {
            task: "java".to_string(),
            current: total_size,
            total: total_size,
            message: format!("Extracting Java {major_version}..."),
        },
    );

    let target_dir = runtimes_dir.join(format!("java-{major_version}"));
    if target_dir.exists() {
        fs::remove_dir_all(&target_dir)?;
    }
    fs::create_dir_all(&target_dir)?;

    let zip_file = fs::File::open(&zip_path)?;
    let mut archive = zip::ZipArchive::new(zip_file)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let out_path = match entry.enclosed_name() {
            Some(p) => target_dir.join(p),
            None => continue,
        };
        if entry.is_dir() {
            fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut outfile = fs::File::create(&out_path)?;
            io::copy(&mut entry, &mut outfile)?;
        }
    }

    let _ = fs::remove_file(&zip_path);

    // Find java.exe inside target_dir
    let mut found_java: Option<PathBuf> = None;
    if let Ok(entries) = fs::read_dir(&target_dir) {
        for entry in entries.flatten() {
            let candidate = entry.path().join("bin").join("java.exe");
            if candidate.exists() {
                found_java = Some(candidate);
                break;
            }
        }
    }

    let java_exe = found_java
        .ok_or_else(|| AppError::Message("Extracted archive did not contain bin/java.exe".to_string()))?;

    inspect_java_executable(&java_exe, true)
        .ok_or_else(|| AppError::Message("Failed to verify extracted Java runtime".to_string()))
}
