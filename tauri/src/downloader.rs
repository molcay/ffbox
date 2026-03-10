use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, Emitter};
use reqwest::Client;
use futures_util::StreamExt;
use std::io::Write;
use std::fs;
use std::fs::File;
use std::process::Command;
use crate::Settings;

#[derive(serde::Serialize, Clone)]
pub struct DownloadProgress {
    pub status: String,
    pub percentage: f64,
}

#[derive(serde::Serialize, Clone)]
pub struct DependenciesStatus {
    pub ffmpeg_ready: bool,
    pub ffprobe_ready: bool,
}

pub fn get_local_binary_dir(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("")).join("bin")
}

pub fn resolve_binary_path(binary_name: &str, default_name: &str, app: &AppHandle) -> PathBuf {
    let active_name = if binary_name.trim().is_empty() { default_name } else { binary_name };

    // 1. If absolute/relative path was provided and explicitly exists
    if !active_name.is_empty() && Path::new(active_name).exists() {
        return PathBuf::from(active_name);
    }
    
    // 2. See if system PATH works natively
    if Command::new(binary_name).arg("-version").output().is_ok() {
        return PathBuf::from(binary_name);
    }

    // 3. Fallback to app_data_dir downloads
    let local_dir = get_local_binary_dir(app);
    let os_bin_name = if cfg!(target_os = "windows") {
        let mut name = active_name.to_string();
        if !name.to_lowercase().ends_with(".exe") {
            name.push_str(".exe");
        }
        name
    } else {
        active_name.to_string()
    };

    let local_path = local_dir.join(&os_bin_name);
    if local_path.exists() && Command::new(&local_path).arg("-version").output().is_ok() {
        return local_path;
    }

    // 4. Return the system path if that works natively
    if !active_name.is_empty() && Command::new(active_name).arg("-version").output().is_ok() {
        return PathBuf::from(active_name);
    }

    // Return the original default string to let it naturally fail execution later
    PathBuf::from(active_name)
}

pub fn check_binary(binary_name: &str, default_name: &str, app: &AppHandle) -> bool {
    let resolved = resolve_binary_path(binary_name, default_name, app);

    let mut cmd = std::process::Command::new(resolved);
    cmd.arg("-version");

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    cmd.output().is_ok()
}

#[tauri::command]
pub async fn check_dependencies(app: tauri::AppHandle, settings: Settings) -> Result<DependenciesStatus, String> {
    let ffmpeg_ready = check_binary(&settings.ffmpeg_path, "ffmpeg", &app);
    let ffprobe_ready = check_binary(&settings.ffprobe_path, "ffprobe", &app);

    Ok(DependenciesStatus {
        ffmpeg_ready,
        ffprobe_ready,
    })
}

fn extract_zip_file(zip_path: &Path, extract_to: &Path, target_file: &str) -> Result<(), String> {
    let file = File::open(zip_path).map_err(|e| format!("Zip Open Error: {}", e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Zip Parsing Error: {}", e))?;

    let mut found = false;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        
        // Skip directories inside the zip
        if (*file.name()).ends_with('/') {
            continue;
        }

        // Check if the filename ends with the target we actually want (e.g., ffmpeg.exe)
        // Some zips have them nested like `ffmpeg-6.1-win-64/bin/ffmpeg.exe`
        if let Some(path) = file.enclosed_name() {
            if let Some(file_name) = path.file_name() {
                if file_name.to_string_lossy() == target_file {
                    let outpath = extract_to.join(target_file);
                    let mut outfile = File::create(&outpath).map_err(|e| e.to_string())?;
                    std::io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;
                    found = true;
                    break;
                }
            }
        }
    }

    if !found {
        return Err(format!("Could not find {} exactly inside the downloaded zip.", target_file));
    }
    Ok(())
}

#[tauri::command]
pub async fn download_ffmpeg(app: tauri::AppHandle) -> Result<(), String> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    let urls = match (os, arch) {
        ("windows", "x86_64") => vec![
            ("https://github.com/ffbinaries/ffbinaries-prebuilt/releases/download/v6.1/ffmpeg-6.1-win-64.zip", "ffmpeg.exe"),
            ("https://github.com/ffbinaries/ffbinaries-prebuilt/releases/download/v6.1/ffprobe-6.1-win-64.zip", "ffprobe.exe"),
        ],
        ("macos", "x86_64") | ("macos", "aarch64") => vec![
            ("https://github.com/ffbinaries/ffbinaries-prebuilt/releases/download/v6.1/ffmpeg-6.1-macos-64.zip", "ffmpeg"),
            ("https://github.com/ffbinaries/ffbinaries-prebuilt/releases/download/v6.1/ffprobe-6.1-macos-64.zip", "ffprobe"),
        ],
        ("linux", "x86_64") => vec![
            ("https://github.com/ffbinaries/ffbinaries-prebuilt/releases/download/v6.1/ffmpeg-6.1-linux-64.zip", "ffmpeg"),
            ("https://github.com/ffbinaries/ffbinaries-prebuilt/releases/download/v6.1/ffprobe-6.1-linux-64.zip", "ffprobe"),
        ],
        ("linux", "aarch64") => vec![
            ("https://github.com/ffbinaries/ffbinaries-prebuilt/releases/download/v6.1/ffmpeg-6.1-linux-arm-64.zip", "ffmpeg"),
            ("https://github.com/ffbinaries/ffbinaries-prebuilt/releases/download/v6.1/ffprobe-6.1-linux-arm-64.zip", "ffprobe"),
        ],
        _ => return Err(format!("Unsupported OS/architecture combo: {}/{}", os, arch)),
    };

    let local_dir = get_local_binary_dir(&app);
    fs::create_dir_all(&local_dir).map_err(|e| e.to_string())?;

    for (url, exe_name) in urls {
        let _ = app.emit("download_progress", DownloadProgress {
            status: format!("Downloading {}...", exe_name),
            percentage: 0.0,
        });

        let client = Client::builder().user_agent("FFBox/1.0").build().unwrap();
        let res = client.get(url).send().await.map_err(|e| format!("Download Request Error: {}", e))?;
        let total_size = res.content_length().unwrap_or(0);
        
        let tmp_zip = local_dir.join(format!("{}.zip", exe_name));
        let mut file = File::create(&tmp_zip).map_err(|e| e.to_string())?;
        
        let mut downloaded: u64 = 0;
        let mut stream = res.bytes_stream();
        
        while let Some(item) = stream.next().await {
            let chunk = item.map_err(|e| e.to_string())?;
            file.write_all(&chunk).map_err(|e| e.to_string())?;
            downloaded += chunk.len() as u64;
            
            if total_size > 0 {
                let _ = app.emit("download_progress", DownloadProgress {
                    status: format!("Downloading {}...", exe_name),
                    percentage: (downloaded as f64 / total_size as f64) * 100.0,
                });
            }
        }
        
        let _ = app.emit("download_progress", DownloadProgress {
            status: format!("Extracting {}...", exe_name),
            percentage: 100.0,
        });

        extract_zip_file(&tmp_zip, &local_dir, exe_name)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let exe_path = local_dir.join(exe_name);
            if exe_path.exists() {
                if let Ok(metadata) = fs::metadata(&exe_path) {
                    let mut perms = metadata.permissions();
                    perms.set_mode(0o755);
                    let _ = fs::set_permissions(&exe_path, perms);
                }
            }
        }
        
        let _ = fs::remove_file(tmp_zip);
    }

    let os_suffix = if cfg!(target_os = "windows") { ".exe" } else { "" };
    let final_ffmpeg = local_dir.join(format!("ffmpeg{}", os_suffix)).to_string_lossy().to_string();
    let final_ffprobe = local_dir.join(format!("ffprobe{}", os_suffix)).to_string_lossy().to_string();

    let mut settings = crate::get_settings();
    settings.ffmpeg_path = final_ffmpeg;
    settings.ffprobe_path = final_ffprobe;
    let _ = crate::save_settings(settings);

    let _ = app.emit("download_progress", DownloadProgress {
        status: "Dependencies installed successfully.".to_string(),
        percentage: 100.0,
    });

    Ok(())
}
