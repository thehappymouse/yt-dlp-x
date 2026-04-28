mod utils;
mod yt_dlp_args;
mod yt_dlp_progress;

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
use yt_dlp_args::{
    build_yt_dlp_args, BuildYtDlpArgsInput, DownloadModeArg, DownloadTuning, VideoQualityArg,
};
use yt_dlp_progress::parse_progress_line;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct YtDlpStatus {
    installed: bool,
    path: Option<String>,
    source: Option<String>,
    version: Option<String>,
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
    filename_template: Option<String>,
    retries: Option<u32>,
    fragment_retries: Option<u32>,
    file_access_retries: Option<u32>,
    concurrent_fragments: Option<u32>,
    retry_sleep: Option<String>,
}

#[derive(Deserialize, Clone, Copy)]
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
            version: yt_dlp::get_version(&path).ok(),
        },
        None => YtDlpStatus {
            installed: false,
            path: None,
            source: None,
            version: None,
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
        version: yt_dlp::get_version(&path).ok(),
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
        filename_template,
        retries,
        fragment_retries,
        file_access_retries,
        concurrent_fragments,
        retry_sleep,
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
    let runtime_caps = yt_dlp::detect_capabilities(&binary_path);

    let output_dir = output_dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(yt_dlp::default_download_dir);

    fs::create_dir_all(&output_dir)
        .await
        .map_err(|err| format!("无法创建下载目录: {err}"))?;

    let temp_dir = output_dir.join(".yt-dlp-temp");
    if runtime_caps.supports_paths_temp {
        fs::create_dir_all(&temp_dir)
            .await
            .map_err(|err| format!("无法创建临时目录: {err}"))?;
    }

    let ffmpeg_path = match mode {
        DownloadMode::Audio => {
            let (path, _) = ffmpeg::ensure_available()?;
            Some(path)
        }
        DownloadMode::Video => ffmpeg::detect_existing()?.map(|(path, _)| path),
    };

    let mode_arg = match mode {
        DownloadMode::Audio => DownloadModeArg::Audio,
        DownloadMode::Video => DownloadModeArg::Video,
    };

    let quality_arg = match quality {
        VideoQuality::Low => VideoQualityArg::Low,
        VideoQuality::Medium => VideoQualityArg::Medium,
        VideoQuality::Highest => VideoQualityArg::Highest,
    };

    let tuning = DownloadTuning::with_overrides(
        filename_template.as_deref(),
        retries,
        fragment_retries,
        file_access_retries,
        concurrent_fragments,
        retry_sleep.as_deref(),
    );

    let args = build_yt_dlp_args(BuildYtDlpArgsInput {
        url: &url,
        mode: mode_arg,
        browser: browser.as_deref(),
        output_dir: &output_dir,
        temp_dir: Some(&temp_dir),
        quality: quality_arg,
        ffmpeg_path: ffmpeg_path.as_deref(),
        runtime_caps: &runtime_caps,
        tuning,
    });

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
            if let Err(err) = window.emit(
                "download-progress",
                json!({
                    "sessionId": session_id.as_ref(),
                    "percent": progress.percent,
                    "percentText": progress.percent_str,
                    "eta": progress.eta,
                    "speed": progress.speed,
                    "total": progress.total,
                    "status": progress.status,
                    "raw": progress.raw,
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

fn path_to_string(path: &Path) -> String {
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
