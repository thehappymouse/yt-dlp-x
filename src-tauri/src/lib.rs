mod utils;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::{fs, process::Command};
use utils::yt_dlp::{self, BinarySource};
use which::which;

const DEFAULT_COOKIE_BROWSERS: &str = "chrome,edge,firefox,brave";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct YtDlpStatus {
    installed: bool,
    path: Option<String>,
    source: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DownloadRequest {
    url: String,
    mode: DownloadMode,
    browser: Option<String>,
    output_dir: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum DownloadMode {
    Audio,
    Video,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadResponse {
    success: bool,
    stdout: String,
    stderr: String,
    output_dir: String,
}

#[tauri::command]
async fn check_yt_dlp() -> Result<YtDlpStatus, String> {
    let status = match yt_dlp::detect_existing()? {
        Some((path, source)) => YtDlpStatus {
            installed: true,
            path: Some(path_to_string(&path)),
            source: Some(source_label(source)),
        },
        None => YtDlpStatus {
            installed: false,
            path: None,
            source: None,
        },
    };

    Ok(status)
}

#[tauri::command]
async fn install_yt_dlp() -> Result<YtDlpStatus, String> {
    let path = yt_dlp::install_latest().await?;
    Ok(YtDlpStatus {
        installed: true,
        path: Some(path_to_string(&path)),
        source: Some(source_label(BinarySource::Bundled)),
    })
}

#[tauri::command]
async fn download_media(request: DownloadRequest) -> Result<DownloadResponse, String> {
    let url = request.url.trim();
    if url.is_empty() {
        return Err("请输入有效的视频链接".into());
    }

    let (binary_path, _) = yt_dlp::ensure_available().await?;

    let mut output_dir = request
        .output_dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(yt_dlp::default_download_dir);

    fs::create_dir_all(&output_dir)
        .await
        .map_err(|err| format!("无法创建下载目录: {err}"))?;

    let mut args: Vec<String> = vec![
        "--newline".into(),
        "--no-progress".into(),
        "--no-playlist".into(),
        "--continue".into(),
        "--no-mtime".into(),
        "-o".into(),
        "%(title)s.%(ext)s".into(),
        "-P".into(),
        output_dir.to_string_lossy().to_string(),
    ];

    let is_youtube = url.contains("youtube.com") || url.contains("youtu.be");
    if is_youtube {
        let browser = request
            .browser
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(DEFAULT_COOKIE_BROWSERS);
        args.push("--cookies-from-browser".into());
        args.push(browser.to_string());
    }

    match request.mode {
        DownloadMode::Audio => {
            ensure_ffmpeg_available()?;
            args.push("-f".into());
            args.push("bestaudio/best".into());
            args.push("-x".into());
            args.push("--audio-format".into());
            args.push("mp3".into());
            args.push("--embed-thumbnail".into());
            args.push("--convert-thumbnails".into());
            args.push("jpg".into());
        }
        DownloadMode::Video => {
            args.push("-f".into());
            args.push("bv*+ba/b".into());
            args.push("--merge-output-format".into());
            args.push("mp4".into());
        }
    }

    args.push(url.to_string());

    let output = Command::new(&binary_path)
        .args(&args)
        .output()
        .await
        .map_err(|err| format!("执行 yt-dlp 失败: {err}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    Ok(DownloadResponse {
        success: output.status.success(),
        stdout,
        stderr,
        output_dir: path_to_string(&output_dir),
    })
}

#[tauri::command]
async fn get_default_download_dir() -> Result<String, String> {
    Ok(path_to_string(&yt_dlp::default_download_dir()))
}

fn ensure_ffmpeg_available() -> Result<(), String> {
    if detect_ffmpeg() {
        Ok(())
    } else {
        Err("未检测到系统 ffmpeg，请先安装后再试，以便下载音频并嵌入封面。".into())
    }
}

fn detect_ffmpeg() -> bool {
    if which("ffmpeg").is_ok() {
        return true;
    }

    #[cfg(target_os = "windows")]
    {
        if which("ffmpeg.exe").is_ok() {
            return true;
        }
    }

    false
}

fn path_to_string(path: &PathBuf) -> String {
    path.to_string_lossy().to_string()
}

fn source_label(source: BinarySource) -> String {
    match source {
        BinarySource::System => "system".into(),
        BinarySource::Bundled => "bundled".into(),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            check_yt_dlp,
            install_yt_dlp,
            download_media,
            get_default_download_dir
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
