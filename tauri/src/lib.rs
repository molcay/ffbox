use serde::{Deserialize, Serialize};
use std::fs;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};
use regex::Regex;

pub mod downloader;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Preset {
    pub name: String,
    pub extension: String,
    pub args: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PresetsConfig {
    pub presets: Vec<Preset>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Settings {
    pub ffmpeg_path: String,
    pub ffprobe_path: String,
    pub default_output_relative: bool,
    pub default_relative_dir_name: String,
    pub default_custom_output_path: String,
    pub default_enable_suffix: bool,
    pub default_suffix: String,
}

impl Default for Settings {
    fn default() -> Self {
        // Platform agnostic default folder path
        let home_dir = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/"));
        let default_ffbox = home_dir.join("FFBox").to_string_lossy().to_string();

        Self {
            ffmpeg_path: "ffmpeg".to_string(),
            ffprobe_path: "ffprobe".to_string(),
            default_output_relative: true,
            default_relative_dir_name: "FFBox".to_string(),
            default_custom_output_path: default_ffbox,
            default_enable_suffix: false,
            default_suffix: "_converted".to_string(),
        }
    }
}

use tauri::{
    Emitter,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
fn get_config_dir() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/"))
        .join(".ffbox")
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn get_presets(_app: tauri::AppHandle) -> Result<PresetsConfig, String> {
    let file_path = get_config_dir().join("presets.toml");
    

    match fs::read_to_string(&file_path) {
        Ok(contents) => {
            match toml::from_str::<PresetsConfig>(&contents) {
                Ok(config) => Ok(config),
                Err(e) => Err(format!("Failed to parse presets.toml: {}", e)),
            }
        }
        Err(e) => Err(format!("Failed to read {}: {}", file_path.display(), e)),
    }
}

fn get_settings_path() -> std::path::PathBuf {
    get_config_dir().join("settings.toml")
}

#[tauri::command]
fn get_settings() -> Settings {
    let file_path = get_settings_path();
    
    match fs::read_to_string(&file_path) {
        Ok(contents) => match toml::from_str::<Settings>(&contents) {
            Ok(settings) => settings,
            Err(_) => Settings::default(),
        },
        Err(_) => Settings::default(),
    }
}

#[tauri::command]
fn save_settings(settings: Settings) -> Result<(), String> {
    let config_dir = get_config_dir();
    let _ = fs::create_dir_all(&config_dir); // Ensure `.ffbox` folder optionally exists before writing
    
    let file_path = config_dir.join("settings.toml");
    
    let contents = toml::to_string_pretty(&settings)
        .map_err(|e| format!("Serialization error: {}", e))?;
        
    fs::write(&file_path, contents)
        .map_err(|e| format!("Failed to write settings.toml: {}", e))
}

#[tauri::command]
fn save_presets(config: PresetsConfig) -> Result<(), String> {
    let config_dir = get_config_dir();
    let _ = fs::create_dir_all(&config_dir);
    
    let file_path = config_dir.join("presets.toml");
    
    let contents = toml::to_string_pretty(&config)
        .map_err(|e| format!("Serialization error: {}", e))?;
        
    fs::write(&file_path, contents)
        .map_err(|e| format!("Failed to write presets.toml: {}", e))
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OutputConfig {
    pub is_relative: bool,
    pub relative_dir_name: String,
    pub custom_dir_path: Option<String>,
    pub apply_suffix: bool,
    pub name_suffix: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConversionPayload {
    pub files: Vec<String>,
    pub preset: Preset,
    pub output_config: OutputConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProgressEvent {
    pub file: String,
    pub percentage: f64,
    pub status: String,
}

// Helper function to recursively find media files
fn find_media_files(path: &std::path::Path, results: &mut Vec<String>) {
    let valid_extensions = ["mp4", "mkv", "avi", "mov", "mp3", "wav", "flac"];
    
    if path.is_file() {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if valid_extensions.contains(&ext.to_lowercase().as_str()) {
                results.push(path.to_string_lossy().to_string());
            }
        }
    } else if path.is_dir() {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                find_media_files(&entry.path(), results);
            }
        }
    }
}

#[tauri::command]
async fn expand_media_paths(paths: Vec<String>) -> Result<Vec<String>, String> {
    let mut expanded = Vec::new();
    for path_str in paths {
        let path = std::path::Path::new(&path_str);
        find_media_files(path, &mut expanded);
    }
    // Deduplicate paths
    expanded.sort();
    expanded.dedup();
    
    Ok(expanded)
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
async fn start_conversion(app: tauri::AppHandle, payload: ConversionPayload) -> Result<(), String> {
    let settings = get_settings();
    let ffmpeg_cmd = downloader::resolve_binary_path(&settings.ffmpeg_path, "ffmpeg", &app);
    let ffprobe_cmd = downloader::resolve_binary_path(&settings.ffprobe_path, "ffprobe", &app);
    
    tokio::spawn(async move {
        for file in payload.files {
            let _ = app.emit("conversion_progress", ProgressEvent {
                file: file.clone(),
                percentage: 0.0,
                status: "Probing file...".to_string(),
            });

            // Run ffprobe to get total duration in seconds
            let mut total_duration_secs = 0.0;
            let mut std_cmd = std::process::Command::new(&ffprobe_cmd);
            std_cmd.args(&[
                "-v", "error",
                "-show_entries", "format=duration",
                "-of", "default=noprint_wrappers=1:nokey=1",
                &file,
            ]);

            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                std_cmd.creation_flags(CREATE_NO_WINDOW);
            }

            let mut async_cmd = tokio::process::Command::from(std_cmd);
            let probe_result = async_cmd.output().await;

            if let Ok(output) = probe_result {
                if let Ok(duration_str) = String::from_utf8(output.stdout) {
                    if let Ok(parsed) = duration_str.trim().parse::<f64>() {
                        total_duration_secs = parsed;
                    }
                }
            }
            println!("Total duration for {}: {} secs", file, total_duration_secs);

            let _ = app.emit("conversion_progress", ProgressEvent {
                file: file.clone(),
                percentage: 0.0,
                status: "Starting conversion...".to_string(),
            });

            // Create the output path based on configuration
            let path = std::path::Path::new(&file);
            let mut stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "output".to_string());
            
            if payload.output_config.apply_suffix {
                if let Some(suffix) = &payload.output_config.name_suffix {
                    stem = format!("{}{}", stem, suffix);
                }
            }

            let mut out_path = file.clone();
            let out_filename = format!("{}.{}", stem, payload.preset.extension);

            if payload.output_config.is_relative {
                if let Some(parent) = path.parent() {
                    let batch_dir = parent.join(&payload.output_config.relative_dir_name);
                    // Ensure the relative subfolder exists
                    let _ = std::fs::create_dir_all(&batch_dir);
                    out_path = batch_dir.join(out_filename).to_string_lossy().to_string();
                }
            } else {
                if let Some(custom_dir) = &payload.output_config.custom_dir_path {
                    let dir_path = std::path::Path::new(custom_dir);
                    let _ = std::fs::create_dir_all(dir_path);
                    out_path = dir_path.join(out_filename).to_string_lossy().to_string();
                }
            }

            // Construct ffmpeg args
            let mut args = vec!["-y", "-i", &file];
            args.extend(payload.preset.args.iter().map(|s| s.as_str()));
            args.push(&out_path);

            let _ = app.emit("conversion_progress", ProgressEvent {
                file: file.clone(),
                percentage: 10.0,
                status: format!("Running ffmpeg..."),
            });

            // Try to run ffmpeg 
            let mut cmd = tokio::process::Command::new(&ffmpeg_cmd);
            cmd.args(&args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            
            #[cfg(target_os = "windows")]
            {
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }

            let mut child = match cmd.spawn() {
                Ok(child) => child,
                Err(e) => {
                    let _ = app.emit("conversion_progress", ProgressEvent {
                        file: file.clone(),
                        percentage: 0.0,
                        status: format!("Failed to start ffmpeg: {}", e),
                    });
                    continue;
                }
            };

            // Parse stderr for progress
            let stderr = child.stderr.take().expect("Failed to grab stderr");
            let mut reader = BufReader::new(stderr).lines();
            
            // matches time=00:00:15.53
            let re = Regex::new(r"time=(\d{2}):(\d{2}):(\d{2}\.\d+)").unwrap();

            while let Ok(Some(line)) = reader.next_line().await {
                if let Some(caps) = re.captures(&line) {
                    if let (Some(h), Some(m), Some(s)) = (caps.get(1), caps.get(2), caps.get(3)) {
                        let hours: f64 = h.as_str().parse().unwrap_or(0.0);
                        let mins: f64 = m.as_str().parse().unwrap_or(0.0);
                        let secs: f64 = s.as_str().parse().unwrap_or(0.0);
                        let current_time_secs = (hours * 3600.0) + (mins * 60.0) + secs;

                        let mut percentage = 0.0;
                        if total_duration_secs > 0.0 {
                            percentage = (current_time_secs / total_duration_secs) * 100.0;
                            // Clamp to max 99.9% while running
                            if percentage > 99.9 { percentage = 99.9; }
                        }

                        let _ = app.emit("conversion_progress", ProgressEvent {
                            file: file.clone(),
                            percentage,
                            status: format!("Converting... {:.1}%", percentage),
                        });
                    }
                }
            }
            
            // Wait for it to finish for this phase
            let result = child.wait().await;
            match result {
                Ok(status) if status.success() => {
                    let _ = app.emit("conversion_progress", ProgressEvent {
                        file: file.clone(),
                        percentage: 100.0,
                        status: "Done".to_string(),
                    });
                }
                _ => {
                    let _ = app.emit("conversion_progress", ProgressEvent {
                        file: file.clone(),
                        percentage: 0.0,
                        status: "Conversion failed".to_string(),
                    });
                }
            }
        }
    });

    Ok(())
}

fn ensure_default_configs() {
    let config_dir = get_config_dir();
    let _ = fs::create_dir_all(&config_dir);

    let settings_path = get_settings_path();
    if !settings_path.exists() {
        let default_settings = Settings::default();
        if let Ok(toml_str) = toml::to_string_pretty(&default_settings) {
            let _ = fs::write(&settings_path, toml_str);
        }
    }

    let presets_path = config_dir.join("presets.toml");
    if !presets_path.exists() {
        let default_presets_str = include_str!("default_presets.toml");
        let _ = fs::write(&presets_path, default_presets_str);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            ensure_default_configs();

            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&quit_i])?;
            let tray = TrayIconBuilder::new()
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => {
                        println!("quit menu item was clicked");
                        app.exit(0);
                    }
                    _ => {
                        println!("menu item {:?} not handled", event.id);
                    }
                })
                .icon(app.default_window_icon().unwrap().clone())
                .build(app)?;
            tray.set_tooltip(Some("FFBox")).unwrap();
            tray.set_title(Some("FFBox")).unwrap();
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_presets, 
            get_settings, 
            save_settings,
            save_presets,
            start_conversion, 
            expand_media_paths,
            downloader::check_dependencies,
            downloader::download_ffmpeg
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
