mod utils;

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{Emitter, Window};
use tokio::{
    fs,
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::Mutex,
};
use utils::{
    ffmpeg::{self, BinarySource as FfmpegBinarySource},
    yt_dlp::{self, BinarySource as YtDlpBinarySource},
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct YtDlpStatus {
    installed: bool,
    path: Option<String>,
    source: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FfmpegStatus {
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
    session_id: Option<String>,
    #[serde(default)]
    quality: VideoQuality,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum DownloadMode {
    Audio,
    Video,
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum VideoQuality {
    Low,
    Medium,
    Highest,
}

impl Default for VideoQuality {
    fn default() -> Self {
        VideoQuality::Highest
    }
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
            source: Some(yt_dlp_source_label(source)),
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
        source: Some(yt_dlp_source_label(YtDlpBinarySource::Bundled)),
    })
}

#[tauri::command]
async fn check_ffmpeg() -> Result<FfmpegStatus, String> {
    let status = match ffmpeg::detect_existing()? {
        Some((path, source)) => FfmpegStatus {
            installed: true,
            path: Some(path_to_string(&path)),
            source: Some(ffmpeg_source_label(source)),
        },
        None => FfmpegStatus {
            installed: false,
            path: None,
            source: None,
        },
    };

    Ok(status)
}

#[tauri::command]
async fn install_ffmpeg() -> Result<FfmpegStatus, String> {
    let path = ffmpeg::install_latest().await?;
    Ok(FfmpegStatus {
        installed: true,
        path: Some(path_to_string(&path)),
        source: Some(ffmpeg_source_label(FfmpegBinarySource::Bundled)),
    })
}

#[tauri::command]
async fn download_media(
    window: Window,
    request: DownloadRequest,
) -> Result<DownloadResponse, String> {
    let DownloadRequest {
        url,
        mode,
        browser,
        output_dir,
        session_id,
        quality,
    } = request;

    let url = url.trim().to_string();
    if url.is_empty() {
        return Err("请输入有效的视频链接".into());
    }

    let session_id = session_id.unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| format!("session-{}", duration.as_millis()))
            .unwrap_or_else(|_| "session-0".into())
    });
    let session_id = Arc::new(session_id);

    let (binary_path, _) = yt_dlp::ensure_available().await?;

    let output_dir = output_dir
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
        if let Some(browser) = browser
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            args.push("--cookies-from-browser".into());
            args.push(browser.to_string());
        }
    }

    let ffmpeg_path = match mode {
        DownloadMode::Audio => {
            let (path, _) = ffmpeg::ensure_available()?;
            args.push("-f".into());
            args.push("bestaudio/best".into());
            args.push("-x".into());
            args.push("--audio-format".into());
            args.push("mp3".into());
            args.push("--embed-thumbnail".into());
            args.push("--convert-thumbnails".into());
            args.push("jpg".into());
            Some(path)
        }
        DownloadMode::Video => {
            args.push("-f".into());
            args.push(video_format_for_quality(quality, &url));
            args.push("--merge-output-format".into());
            args.push("mp4".into());
            ffmpeg::detect_existing()?.map(|(path, _)| path)
        }
    };

    if let Some(ref path) = ffmpeg_path {
        args.push("--ffmpeg-location".into());
        args.push(path_to_string(path));
    }

    args.push(url.clone());

    let mut command = Command::new(&binary_path);
    command.args(&args);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    command.kill_on_drop(true);

    let mut child = command
        .spawn()
        .map_err(|err| format!("执行 yt-dlp 失败: {err}"))?;

    let stdout_buffer = Arc::new(Mutex::new(Vec::new()));
    let stderr_buffer = Arc::new(Mutex::new(Vec::new()));

    let stdout_task = if let Some(stdout) = child.stdout.take() {
        let window = window.clone();
        let session_id = Arc::clone(&session_id);
        let buffer = Arc::clone(&stdout_buffer);
        Some(tokio::spawn(async move {
            forward_stream(stdout, window, session_id, "stdout", buffer).await
        }))
    } else {
        None
    };

    let stderr_task = if let Some(stderr) = child.stderr.take() {
        let window = window.clone();
        let session_id = Arc::clone(&session_id);
        let buffer = Arc::clone(&stderr_buffer);
        Some(tokio::spawn(async move {
            forward_stream(stderr, window, session_id, "stderr", buffer).await
        }))
    } else {
        None
    };

    let status = child
        .wait()
        .await
        .map_err(|err| format!("等待 yt-dlp 结束失败: {err}"))?;

    if let Some(task) = stdout_task {
        match task.await {
            Ok(Ok(())) => {}
            Ok(Err(err)) => eprintln!("读取 yt-dlp 标准输出失败: {err}"),
            Err(err) => eprintln!("读取 yt-dlp 标准输出任务失败: {err}"),
        }
    }

    if let Some(task) = stderr_task {
        match task.await {
            Ok(Ok(())) => {}
            Ok(Err(err)) => eprintln!("读取 yt-dlp 标准错误失败: {err}"),
            Err(err) => eprintln!("读取 yt-dlp 标准错误任务失败: {err}"),
        }
    }

    let stdout = {
        let lines = stdout_buffer.lock().await;
        lines.join("\n")
    };
    let stdout = stdout.trim().to_string();

    let stderr = {
        let lines = stderr_buffer.lock().await;
        lines.join("\n")
    };
    let stderr = stderr.trim().to_string();

    Ok(DownloadResponse {
        success: status.success(),
        stdout,
        stderr,
        output_dir: path_to_string(&output_dir),
    })
}

fn video_format_for_quality(quality: VideoQuality, url: &str) -> String {
    match quality {
        VideoQuality::Low => "bv*[height<=480]+ba/b[height<=480]/bv*[height<=720]+ba/b[height<=720]/worst".into(),
        VideoQuality::Medium => "bv*[height<=1080]+ba/b[height<=1080]/bv*[height<=720]+ba/b[height<=720]/b".into(),
        VideoQuality::Highest => {
            if is_bilibili_url(url) {
                "bv*[height>=2160]+ba/bv*[height>=1080]+ba/bv*+ba/b".into()
            } else {
                "bv*+ba/b".into()
            }
        }
    }
}

fn is_bilibili_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.contains("bilibili.com")
        || lower.contains("b23.tv")
        || lower.contains("bilivideo.com")
        || lower.contains("acg.tv")
}

struct ProgressInfo {
    percent: f64,
    percent_str: String,
    eta: Option<String>,
    speed: Option<String>,
    total: Option<String>,
    status: Option<String>,
    raw: String,
}

fn parse_progress_line(line: &str) -> Option<ProgressInfo> {
    if !line.starts_with("[download]") {
        return None;
    }

    let trimmed = line.trim_start_matches("[download]").trim();
    let (percent_part, rest_part) = trimmed.split_once('%')?;
    let percent_str = percent_part.trim();
    if percent_str.is_empty() {
        return None;
    }

    let percent_value = percent_str.parse::<f64>().ok()?;
    let percent_display = format!("{percent_str}%");
    let rest = rest_part.trim();

    let mut eta = None;

    if let Some(index) = rest.rfind("ETA ") {
        let value = rest[index + 4..].trim();
        if !value.is_empty() {
            eta = Some(value.to_string());
        }
    } else if let Some(index) = rest.rfind(" in ") {
        let value = rest[index + 4..].trim();
        if !value.is_empty() {
            eta = Some(value.to_string());
        }
    }

    let mut speed = None;
    if let Some(index) = rest.find(" at ") {
        let after = &rest[index + 4..];
        if let Some(token) = after.split_whitespace().next() {
            let cleaned = token.trim().trim_end_matches(',');
            if !cleaned.is_empty() {
                speed = Some(cleaned.to_string());
            }
        }
    }

    let mut total = None;
    if let Some(after_of) = rest.trim_start().strip_prefix("of ") {
        let mut end_index = after_of.len();
        for marker in [" at ", " ETA ", " in "] {
            if let Some(idx) = after_of.find(marker) {
                end_index = end_index.min(idx);
            }
        }
        let candidate = after_of[..end_index].trim().trim_end_matches(',');
        if !candidate.is_empty() {
            total = Some(candidate.to_string());
        }
    }

    let mut status = None;
    if rest.contains(" in ") || percent_value >= 100.0 {
        status = Some("finished".to_string());
    } else if rest.contains("ETA") || !rest.is_empty() {
        status = Some("downloading".to_string());
    }

    Some(ProgressInfo {
        percent: percent_value,
        percent_str: percent_display,
        eta,
        speed,
        total,
        status,
        raw: trimmed.to_string(),
    })
}

async fn forward_stream<R>(
    reader: R,
    window: Window,
    session_id: Arc<String>,
    stream: &'static str,
    buffer: Arc<Mutex<Vec<String>>>,
) -> Result<(), std::io::Error>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    while let Some(line) = lines.next_line().await? {
        {
            let mut entries = buffer.lock().await;
            entries.push(line.clone());
        }

        if let Err(err) = window.emit(
            "download-log",
            json!({
                "sessionId": session_id.as_ref(),
                "stream": stream,
                "line": line,
            }),
        ) {
            eprintln!("Failed to emit log event: {err}");
        }

        if let Some(progress) = parse_progress_line(&line) {
            let ProgressInfo {
                percent,
                percent_str,
                eta,
                speed,
                total,
                status,
                raw,
            } = progress;

            if let Err(err) = window.emit(
                "download-progress",
                json!({
                    "sessionId": session_id.as_ref(),
                    "percent": percent,
                    "percentText": percent_str,
                    "eta": eta,
                    "speed": speed,
                    "total": total,
                    "status": status,
                    "raw": raw,
                }),
            ) {
                eprintln!("Failed to emit progress event: {err}");
            }
        }
    }

    Ok(())
}

#[tauri::command]
async fn get_default_download_dir() -> Result<String, String> {
    Ok(path_to_string(&yt_dlp::default_download_dir()))
}

#[tauri::command]
async fn open_directory(path: String) -> Result<(), String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("请输入有效的目录路径".into());
    }

    let resolved_path = expand_user_path(trimmed)?;

    if !resolved_path.exists() {
        return Err("目标路径不存在".into());
    }

    let canonical_path = resolved_path
        .canonicalize()
        .map_err(|err| format!("解析路径失败: {err}"))?;

    if !canonical_path.is_dir() {
        return Err("仅支持打开目录路径".into());
    }

    let target = canonical_path.clone();

    let open_result = tauri::async_runtime::spawn_blocking(move || open_in_file_manager(&target))
        .await
        .map_err(|err| format!("打开目录失败: {err}"))?;

    open_result?;

    Ok(())
}

fn expand_user_path(input: &str) -> Result<PathBuf, String> {
    if input == "~" {
        user_home_dir().ok_or_else(|| "无法定位用户主目录".to_string())
    } else if let Some(stripped) = input
        .strip_prefix("~/")
        .or_else(|| input.strip_prefix("~\\"))
    {
        user_home_dir()
            .map(|home| home.join(stripped))
            .ok_or_else(|| "无法定位用户主目录".to_string())
    } else {
        Ok(PathBuf::from(input))
    }
}

fn user_home_dir() -> Option<PathBuf> {
    directories_next::BaseDirs::new().map(|base| base.home_dir().to_path_buf())
}

#[cfg(target_os = "macos")]
fn open_in_file_manager(path: &Path) -> Result<(), String> {
    let status = std::process::Command::new("open")
        .arg(path)
        .status()
        .map_err(|err| format!("执行 open 命令失败: {err}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "打开目录失败，退出代码: {}",
            exit_status_message(status)
        ))
    }
}

#[cfg(target_os = "windows")]
fn open_in_file_manager(path: &Path) -> Result<(), String> {
    use std::ffi::OsString;

    let mut command = std::process::Command::new("explorer");
    if path.is_dir() {
        command.arg(path);
    } else {
        let mut argument = OsString::from("/select,");
        argument.push(path);
        command.arg(argument);
    }

    let status = command
        .status()
        .map_err(|err| format!("执行 explorer 命令失败: {err}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "打开目录失败，退出代码: {}",
            exit_status_message(status)
        ))
    }
}

#[cfg(target_os = "linux")]
fn open_in_file_manager(path: &Path) -> Result<(), String> {
    let status = std::process::Command::new("xdg-open")
        .arg(path)
        .status()
        .map_err(|err| format!("执行 xdg-open 命令失败: {err}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "打开目录失败，退出代码: {}",
            exit_status_message(status)
        ))
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn open_in_file_manager(_path: &Path) -> Result<(), String> {
    Err("当前平台暂不支持打开目录".into())
}

fn exit_status_message(status: std::process::ExitStatus) -> String {
    status
        .code()
        .map(|code| code.to_string())
        .unwrap_or_else(|| "未知".into())
}

fn path_to_string(path: &PathBuf) -> String {
    path.to_string_lossy().to_string()
}

fn yt_dlp_source_label(source: YtDlpBinarySource) -> String {
    match source {
        YtDlpBinarySource::System => "system".into(),
        YtDlpBinarySource::Bundled => "bundled".into(),
    }
}

fn ffmpeg_source_label(source: FfmpegBinarySource) -> String {
    match source {
        FfmpegBinarySource::System => "system".into(),
        FfmpegBinarySource::Bundled => "bundled".into(),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            check_yt_dlp,
            check_ffmpeg,
            install_yt_dlp,
            install_ffmpeg,
            download_media,
            get_default_download_dir,
            open_directory
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
