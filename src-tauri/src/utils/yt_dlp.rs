use directories_next::{BaseDirs, ProjectDirs, UserDirs};
use std::path::{Path, PathBuf};
use tokio::fs;
use which::which;

#[derive(Debug, Clone, Copy)]
pub enum BinarySource {
    System,
    Bundled,
}

pub fn detect_existing() -> Result<Option<(PathBuf, BinarySource)>, String> {
    if let Some(path) = detect_system_binary() {
        return Ok(Some((path, BinarySource::System)));
    }

    if let Some(path) = detect_bundled_binary()? {
        return Ok(Some((path, BinarySource::Bundled)));
    }

    Ok(None)
}

pub async fn ensure_available() -> Result<(PathBuf, BinarySource), String> {
    if let Some(path) = detect_system_binary() {
        return Ok((path, BinarySource::System));
    }

    if let Some(path) = detect_bundled_binary()? {
        ensure_executable_permissions(&path).await?;
        return Ok((path, BinarySource::Bundled));
    }

    let path = bundled_binary_path()?;
    download_to(&path).await?;
    Ok((path, BinarySource::Bundled))
}

pub async fn install_latest() -> Result<PathBuf, String> {
    let path = bundled_binary_path()?;
    download_to(&path).await?;
    Ok(path)
}

pub fn default_download_dir() -> PathBuf {
    if let Some(user_dirs) = UserDirs::new() {
        if let Some(download_dir) = user_dirs.download_dir() {
            return download_dir.to_path_buf();
        }
    }

    if let Some(base_dirs) = BaseDirs::new() {
        return base_dirs.home_dir().join("Downloads");
    }

    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn detect_system_binary() -> Option<PathBuf> {
    if let Ok(path) = which("yt-dlp") {
        return Some(path);
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(path) = which("yt-dlp.exe") {
            return Some(path);
        }
    }

    None
}

fn detect_bundled_binary() -> Result<Option<PathBuf>, String> {
    let path = bundled_binary_path()?;
    if path.exists() {
        Ok(Some(path))
    } else {
        Ok(None)
    }
}

fn bundled_binary_path() -> Result<PathBuf, String> {
    let dirs = project_dirs()?;
    Ok(dirs.data_dir().join("bin").join(binary_file_name()))
}

fn project_dirs() -> Result<ProjectDirs, String> {
    ProjectDirs::from("com", "yt-dlp-x", "yt-dlp-x")
        .ok_or_else(|| "无法定位应用数据目录".to_string())
}

fn binary_file_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    }
}

fn download_url() -> &'static str {
    if cfg!(target_os = "windows") {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
    } else if cfg!(target_os = "macos") {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos"
    } else {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp"
    }
}

async fn download_to(target_path: &Path) -> Result<(), String> {
    let parent = target_path
        .parent()
        .ok_or_else(|| "无法确定 yt-dlp 存储目录".to_string())?;
    fs::create_dir_all(parent)
        .await
        .map_err(|err| format!("创建目录失败: {err}"))?;

    let response = reqwest::Client::new()
        .get(download_url())
        .send()
        .await
        .map_err(|err| format!("下载 yt-dlp 失败: {err}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "下载 yt-dlp 失败，状态码: {}",
            response.status()
        ));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|err| format!("读取下载内容失败: {err}"))?;

    fs::write(target_path, bytes)
        .await
        .map_err(|err| format!("写入 yt-dlp 文件失败: {err}"))?;

    ensure_executable_permissions(target_path).await?;
    Ok(())
}

#[cfg(unix)]
async fn ensure_executable_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    if path.exists() {
        let permissions = std::fs::Permissions::from_mode(0o755);
        fs::set_permissions(path, permissions)
            .await
            .map_err(|err| format!("设置执行权限失败: {err}"))?;
    }

    Ok(())
}

#[cfg(not(unix))]
async fn ensure_executable_permissions(_path: &Path) -> Result<(), String> {
    Ok(())
}
