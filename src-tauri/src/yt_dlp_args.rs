use std::path::Path;

use crate::{
    utils::yt_dlp::RuntimeCapabilities,
    yt_dlp_progress::progress_template_value,
};

const DOUYIN_REFERER: &str = "https://www.douyin.com/";
const DOUYIN_USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 14_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/14.0 Mobile/15E148 Safari/604.1";

pub const DEFAULT_FILENAME_TEMPLATE: &str = "%(title).150B [%(id)s].%(ext)s";
const DEFAULT_RETRIES: u32 = 10;
const DEFAULT_FRAGMENT_RETRIES: u32 = 10;
const DEFAULT_FILE_ACCESS_RETRIES: u32 = 3;
const DEFAULT_CONCURRENT_FRAGMENTS: u32 = 1;
const DEFAULT_RETRY_SLEEP: &str = "1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadModeArg {
    Audio,
    Video,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoQualityArg {
    Low,
    Medium,
    Highest,
}

#[derive(Debug, Clone)]
pub struct DownloadTuning {
    pub retries: u32,
    pub fragment_retries: u32,
    pub file_access_retries: u32,
    pub concurrent_fragments: u32,
    pub retry_sleep: String,
    pub filename_template: String,
}

impl DownloadTuning {
    pub fn with_overrides(
        filename_template: Option<&str>,
        retries: Option<u32>,
        fragment_retries: Option<u32>,
        file_access_retries: Option<u32>,
        concurrent_fragments: Option<u32>,
        retry_sleep: Option<&str>,
    ) -> Self {
        let filename_template = sanitize_filename_template(filename_template);

        Self {
            retries: retries.unwrap_or(DEFAULT_RETRIES).min(100),
            fragment_retries: fragment_retries
                .unwrap_or(DEFAULT_FRAGMENT_RETRIES)
                .min(100),
            file_access_retries: file_access_retries
                .unwrap_or(DEFAULT_FILE_ACCESS_RETRIES)
                .min(100),
            concurrent_fragments: concurrent_fragments
                .unwrap_or(DEFAULT_CONCURRENT_FRAGMENTS)
                .clamp(1, 16),
            retry_sleep: sanitize_retry_sleep(retry_sleep),
            filename_template,
        }
    }
}

impl Default for DownloadTuning {
    fn default() -> Self {
        Self {
            retries: DEFAULT_RETRIES,
            fragment_retries: DEFAULT_FRAGMENT_RETRIES,
            file_access_retries: DEFAULT_FILE_ACCESS_RETRIES,
            concurrent_fragments: DEFAULT_CONCURRENT_FRAGMENTS,
            retry_sleep: DEFAULT_RETRY_SLEEP.to_string(),
            filename_template: DEFAULT_FILENAME_TEMPLATE.to_string(),
        }
    }
}

pub struct BuildYtDlpArgsInput<'a> {
    pub url: &'a str,
    pub mode: DownloadModeArg,
    pub browser: Option<&'a str>,
    pub output_dir: &'a Path,
    pub temp_dir: Option<&'a Path>,
    pub quality: VideoQualityArg,
    pub ffmpeg_path: Option<&'a Path>,
    pub runtime_caps: &'a RuntimeCapabilities,
    pub tuning: DownloadTuning,
}

pub fn build_yt_dlp_args(input: BuildYtDlpArgsInput<'_>) -> Vec<String> {
    let BuildYtDlpArgsInput {
        url,
        mode,
        browser,
        output_dir,
        temp_dir,
        quality,
        ffmpeg_path,
        runtime_caps,
        tuning,
    } = input;

    let mut args: Vec<String> = vec![
        "--newline".into(),
        "--no-playlist".into(),
        "--continue".into(),
        "--no-mtime".into(),
        "-o".into(),
        tuning.filename_template,
        "-P".into(),
        format!("home:{}", output_dir.to_string_lossy()),
        "-R".into(),
        tuning.retries.to_string(),
        "--fragment-retries".into(),
        tuning.fragment_retries.to_string(),
        "--file-access-retries".into(),
        tuning.file_access_retries.to_string(),
        "--retry-sleep".into(),
        tuning.retry_sleep,
    ];

    if tuning.concurrent_fragments > 1 {
        args.push("-N".into());
        args.push(tuning.concurrent_fragments.to_string());
    }

    if let Some(temp_dir) = temp_dir.filter(|_| runtime_caps.supports_paths_temp) {
        args.push("-P".into());
        args.push(format!("temp:{}", temp_dir.to_string_lossy()));
    }

    if runtime_caps.supports_progress_template {
        args.push("--progress-template".into());
        args.push(progress_template_value().into());
    }

    if let Some(browser) = browser
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "none")
    {
        args.push("--cookies-from-browser".into());
        args.push(browser.to_string());
    }

    match mode {
        DownloadModeArg::Audio => {
            args.push("-f".into());
            args.push("bestaudio/best".into());
            args.push("-x".into());
            args.push("--audio-format".into());
            args.push("mp3".into());
            args.push("--embed-thumbnail".into());
            args.push("--convert-thumbnails".into());
            args.push("jpg".into());
        }
        DownloadModeArg::Video => {
            args.push("-f".into());
            args.push(video_format_for_quality(quality, url));
            args.push("--merge-output-format".into());
            args.push("mp4".into());
        }
    }

    if let Some(path) = ffmpeg_path {
        args.push("--ffmpeg-location".into());
        args.push(path.to_string_lossy().to_string());
    }

    apply_site_specific_overrides(&mut args, url, runtime_caps);

    args.push(url.to_string());
    args
}

fn apply_site_specific_overrides(
    args: &mut Vec<String>,
    url: &str,
    runtime_caps: &RuntimeCapabilities,
) {
    if is_douyin_url(url) {
        if runtime_caps.supports_add_headers {
            args.push("--add-headers".into());
            args.push(format!("Referer: {DOUYIN_REFERER}"));
            args.push("--add-headers".into());
            args.push(format!("User-Agent: {DOUYIN_USER_AGENT}"));
        } else {
            args.push("--referer".into());
            args.push(DOUYIN_REFERER.into());
            args.push("--user-agent".into());
            args.push(DOUYIN_USER_AGENT.into());
        }
    }
}

fn video_format_for_quality(quality: VideoQualityArg, url: &str) -> String {
    match quality {
        VideoQualityArg::Low => "bv*[height<=480]+ba/b[height<=480]/bv*[height<=720]+ba/b[height<=720]/worst".into(),
        VideoQualityArg::Medium => "bv*[height<=1080]+ba/b[height<=1080]/bv*[height<=720]+ba/b[height<=720]/b".into(),
        VideoQualityArg::Highest => {
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

fn is_douyin_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.contains("douyin.com") || lower.contains("iesdouyin.com")
}

fn sanitize_filename_template(value: Option<&str>) -> String {
    let Some(raw) = value else {
        return DEFAULT_FILENAME_TEMPLATE.to_string();
    };

    let candidate = raw.trim();
    if candidate.is_empty()
        || candidate.contains('\n')
        || candidate.contains('\r')
        || candidate.contains('\0')
    {
        return DEFAULT_FILENAME_TEMPLATE.to_string();
    }

    if candidate.contains("%(ext)") {
        candidate.to_string()
    } else {
        format!("{candidate}.%(ext)s")
    }
}

fn sanitize_retry_sleep(value: Option<&str>) -> String {
    let Some(raw) = value else {
        return DEFAULT_RETRY_SLEEP.to_string();
    };

    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.contains('\n') || trimmed.contains('\r') || trimmed.len() > 64 {
        DEFAULT_RETRY_SLEEP.to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        build_yt_dlp_args, BuildYtDlpArgsInput, DownloadModeArg, DownloadTuning, VideoQualityArg,
    };
    use crate::utils::yt_dlp::RuntimeCapabilities;

    fn runtime_caps() -> RuntimeCapabilities {
        RuntimeCapabilities {
            supports_progress_template: true,
            supports_paths_temp: true,
            supports_add_headers: true,
        }
    }

    #[test]
    fn builds_audio_args_with_stable_defaults() {
        let caps = runtime_caps();
        let tuning = DownloadTuning::with_overrides(None, None, None, None, None, None);

        let args = build_yt_dlp_args(BuildYtDlpArgsInput {
            url: "https://www.youtube.com/watch?v=abc",
            mode: DownloadModeArg::Audio,
            browser: Some("chrome"),
            output_dir: Path::new("/tmp/output"),
            temp_dir: Some(Path::new("/tmp/output/.yt-dlp-temp")),
            quality: VideoQualityArg::Highest,
            ffmpeg_path: Some(Path::new("/usr/bin/ffmpeg")),
            runtime_caps: &caps,
            tuning,
        });

        assert!(args.contains(&"--progress-template".to_string()));
        assert!(args.contains(&"--cookies-from-browser".to_string()));
        assert!(args.contains(&"chrome".to_string()));
        assert!(args.contains(&"--audio-format".to_string()));
        assert!(args.contains(&"mp3".to_string()));
        assert!(args.iter().any(|arg| arg == "home:/tmp/output"));
        assert!(args.iter().any(|arg| arg == "temp:/tmp/output/.yt-dlp-temp"));
    }

    #[test]
    fn uses_add_headers_for_douyin_when_supported() {
        let caps = runtime_caps();
        let args = build_yt_dlp_args(BuildYtDlpArgsInput {
            url: "https://www.douyin.com/video/123",
            mode: DownloadModeArg::Video,
            browser: None,
            output_dir: Path::new("/tmp/output"),
            temp_dir: None,
            quality: VideoQualityArg::Highest,
            ffmpeg_path: None,
            runtime_caps: &caps,
            tuning: DownloadTuning::default(),
        });

        assert!(args.contains(&"--add-headers".to_string()));
        assert!(!args.contains(&"--referer".to_string()));
        assert!(!args.contains(&"--user-agent".to_string()));
    }

    #[test]
    fn falls_back_to_legacy_headers_when_add_headers_unavailable() {
        let caps = RuntimeCapabilities {
            supports_progress_template: false,
            supports_paths_temp: false,
            supports_add_headers: false,
        };

        let args = build_yt_dlp_args(BuildYtDlpArgsInput {
            url: "https://www.douyin.com/video/123",
            mode: DownloadModeArg::Video,
            browser: None,
            output_dir: Path::new("/tmp/output"),
            temp_dir: Some(Path::new("/tmp/output/.yt-dlp-temp")),
            quality: VideoQualityArg::Highest,
            ffmpeg_path: None,
            runtime_caps: &caps,
            tuning: DownloadTuning::default(),
        });

        assert!(args.contains(&"--referer".to_string()));
        assert!(args.contains(&"--user-agent".to_string()));
        assert!(!args.contains(&"--add-headers".to_string()));
        assert!(!args.iter().any(|arg| arg.starts_with("temp:")));
    }

    #[test]
    fn filename_template_falls_back_when_empty() {
        let caps = runtime_caps();
        let tuning = DownloadTuning::with_overrides(Some("   "), None, None, None, None, None);

        let args = build_yt_dlp_args(BuildYtDlpArgsInput {
            url: "https://example.com/video",
            mode: DownloadModeArg::Video,
            browser: None,
            output_dir: Path::new("/tmp/output"),
            temp_dir: None,
            quality: VideoQualityArg::Highest,
            ffmpeg_path: None,
            runtime_caps: &caps,
            tuning,
        });

        assert!(args.contains(&"%(title).150B [%(id)s].%(ext)s".to_string()));
    }
}
