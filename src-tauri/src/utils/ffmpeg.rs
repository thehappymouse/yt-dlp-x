use directories_next::ProjectDirs;
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

pub fn ensure_available() -> Result<(PathBuf, BinarySource), String> {
    detect_existing()?
        .ok_or_else(|| "未检测到系统或内置 ffmpeg，请先安装后再试，以便下载音频并嵌入封面。".into())
}

pub async fn install_latest() -> Result<PathBuf, String> {
    if !cfg!(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "linux"
    )) {
        return Err("当前平台暂不支持自动安装 ffmpeg".into());
    }

    let url = download_url()?;
    let path = bundled_binary_path()?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|err| format!("创建目录失败: {err}"))?;
    }

    let response = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .map_err(|err| format!("下载 ffmpeg 失败: {err}"))?;

    if !response.status().is_success() {
        return Err(format!("下载 ffmpeg 失败，状态码: {}", response.status()));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|err| format!("读取 ffmpeg 下载内容失败: {err}"))?
        .to_vec();

    let target_path = path.clone();
    tokio::task::spawn_blocking(move || extract_ffmpeg(bytes, target_path))
        .await
        .map_err(|err| format!("解压 ffmpeg 压缩包失败: {err}"))??;

    ensure_executable_permissions(&path).await?;

    Ok(path)
}

fn detect_system_binary() -> Option<PathBuf> {
    if let Ok(path) = which("ffmpeg") {
        return Some(path);
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(path) = which("ffmpeg.exe") {
            return Some(path);
        }
    }

    super::path_search::locate_macos_binary(&["ffmpeg"])
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
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    }
}

fn download_url() -> Result<&'static str, String> {
    if cfg!(target_os = "windows") {
        Ok("https://github.com/GyanD/codexffmpeg/releases/latest/download/ffmpeg-release-essentials.zip")
    } else if cfg!(target_os = "macos") {
        Ok("https://evermeet.cx/ffmpeg/getrelease/zip")
    } else if cfg!(target_os = "linux") {
        Ok("https://github.com/BtbN/FFmpeg-Builds/releases/latest/download/ffmpeg-master-latest-linux64-gpl.tar.xz")
    } else {
        Err("当前平台暂不支持自动安装 ffmpeg".into())
    }
}

fn extract_ffmpeg(bytes: Vec<u8>, target_path: PathBuf) -> Result<(), String> {
    if cfg!(target_os = "windows") {
        extract_ffmpeg_from_zip(bytes, target_path, "ffmpeg.exe")
    } else if cfg!(target_os = "macos") {
        extract_ffmpeg_from_zip(bytes, target_path, "ffmpeg")
    } else if cfg!(target_os = "linux") {
        extract_ffmpeg_from_tar_xz(bytes, target_path)
    } else {
        Err("当前平台暂不支持自动安装 ffmpeg".into())
    }
}

fn extract_ffmpeg_from_zip(
    bytes: Vec<u8>,
    target_path: PathBuf,
    binary_name: &str,
) -> Result<(), String> {
    use std::io::{Cursor, Read, Write};

    let reader = Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(reader).map_err(|err| format!("解析 ffmpeg 压缩包失败: {err}"))?;

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|err| format!("读取 ffmpeg 压缩包条目失败: {err}"))?;

        if !file.is_file() {
            continue;
        }

        let name = file.name().to_string();
        if name.ends_with(binary_name) {
            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent).map_err(|err| format!("创建目录失败: {err}"))?;
            }

            let mut output = std::fs::File::create(&target_path)
                .map_err(|err| format!("写入 ffmpeg 文件失败: {err}"))?;
            std::io::copy(&mut file, &mut output)
                .map_err(|err| format!("解压 ffmpeg 文件失败: {err}"))?;
            return Ok(());
        }
    }

    Err("未在压缩包中找到 ffmpeg 可执行文件".into())
}

fn extract_ffmpeg_from_tar_xz(bytes: Vec<u8>, target_path: PathBuf) -> Result<(), String> {
    use std::io::{Cursor, Read, Write};

    let cursor = Cursor::new(bytes);
    let decompressor = xz2::read::XzDecoder::new(cursor);
    let mut archive = tar::Archive::new(decompressor);

    let entries = archive
        .entries()
        .map_err(|err| format!("解析 ffmpeg 压缩包失败: {err}"))?;

    for entry_result in entries {
        let mut entry = entry_result.map_err(|err| format!("读取 ffmpeg 压缩包条目失败: {err}"))?;
        let path = entry
            .path()
            .map_err(|err| format!("解析压缩包路径失败: {err}"))?;

        if let Some(name) = path.file_name().and_then(|segment| segment.to_str()) {
            if name == "ffmpeg" {
                if let Some(parent) = target_path.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|err| format!("创建目录失败: {err}"))?;
                }

                let mut output = std::fs::File::create(&target_path)
                    .map_err(|err| format!("写入 ffmpeg 文件失败: {err}"))?;
                std::io::copy(&mut entry, &mut output)
                    .map_err(|err| format!("解压 ffmpeg 文件失败: {err}"))?;
                return Ok(());
            }
        }
    }

    Err("未在压缩包中找到 ffmpeg 可执行文件".into())
}

#[cfg(unix)]
async fn ensure_executable_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    if path.exists() {
        let permissions = std::fs::Permissions::from_mode(0o755);
        fs::set_permissions(path, permissions)
            .await
            .map_err(|err| format!("设置 ffmpeg 执行权限失败: {err}"))?;
    }

    Ok(())
}

#[cfg(not(unix))]
async fn ensure_executable_permissions(_path: &Path) -> Result<(), String> {
    Ok(())
}
