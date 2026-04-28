use directories_next::{BaseDirs, ProjectDirs, UserDirs};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, OnceLock},
};
use tokio::fs;
use which::which;

#[derive(Debug, Clone, Copy)]
pub enum BinarySource {
    System,
    Bundled,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeCapabilities {
    pub supports_progress_template: bool,
    pub supports_paths_temp: bool,
    pub supports_add_headers: bool,
}

pub fn detect_existing() -> Result<Option<(PathBuf, BinarySource)>, String> {
    if let Some(path) = detect_bundled_binary()? {
        if validate_binary(&path).is_ok() {
            return Ok(Some((path, BinarySource::Bundled)));
        }
        let _ = std::fs::remove_file(&path);
    }

    if let Some(path) = detect_system_binary() {
        if validate_binary(&path).is_ok() {
            return Ok(Some((path, BinarySource::System)));
        }
    }

    Ok(None)
}

pub async fn ensure_available() -> Result<(PathBuf, BinarySource), String> {
    if let Some(path) = detect_bundled_binary()? {
        ensure_executable_permissions(&path).await?;
        if validate_binary(&path).is_ok() {
            return Ok((path, BinarySource::Bundled));
        }
        let _ = fs::remove_file(&path).await;
    }

    if let Some(path) = detect_system_binary() {
        if validate_binary(&path).is_ok() {
            return Ok((path, BinarySource::System));
        }
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

pub fn get_version(path: &Path) -> Result<String, String> {
    let output = Command::new(path)
        .arg("--version")
        .output()
        .map_err(|err| format!("执行 yt-dlp --version 失败: {err}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "yt-dlp --version 返回失败状态".to_string()
        } else {
            format!("yt-dlp --version 执行失败: {stderr}")
        });
    }

    let version = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "无法解析 yt-dlp 版本号".to_string())?;

    Ok(version.to_string())
}

pub fn detect_capabilities(path: &Path) -> RuntimeCapabilities {
    let key = path.to_path_buf();
    if let Some(value) = capability_cache()
        .lock()
        .ok()
        .and_then(|cache| cache.get(&key).cloned())
    {
        return value;
    }

    let capabilities = detect_capabilities_inner(path).unwrap_or_default();

    if let Ok(mut cache) = capability_cache().lock() {
        cache.insert(key, capabilities.clone());
    }

    capabilities
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

fn capability_cache() -> &'static Mutex<HashMap<PathBuf, RuntimeCapabilities>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, RuntimeCapabilities>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn detect_capabilities_inner(path: &Path) -> Result<RuntimeCapabilities, String> {
    let output = Command::new(path)
        .arg("--help")
        .output()
        .map_err(|err| format!("执行 yt-dlp --help 失败: {err}"))?;

    if !output.status.success() {
        return Err("yt-dlp --help 返回失败状态".into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let content = format!("{stdout}\n{stderr}");

    Ok(RuntimeCapabilities {
        supports_progress_template: content.contains("--progress-template"),
        supports_paths_temp: content.contains("home,temp")
            || content.contains("temp:")
            || (content.contains("--paths") && content.contains(" temp")),
        supports_add_headers: content.contains("--add-headers"),
    })
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

    super::path_search::locate_macos_binary(&["yt-dlp", "yt-dlp_macos"])
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
    release_asset_name()
}

fn release_asset_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "yt-dlp.exe"
    } else if cfg!(target_os = "macos") {
        "yt-dlp_macos"
    } else {
        "yt-dlp"
    }
}

fn download_url() -> String {
    format!(
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/{}",
        release_asset_name()
    )
}

fn checksums_url() -> &'static str {
    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/SHA2-256SUMS"
}

async fn download_to(target_path: &Path) -> Result<(), String> {
    let parent = target_path
        .parent()
        .ok_or_else(|| "无法确定 yt-dlp 存储目录".to_string())?;
    fs::create_dir_all(parent)
        .await
        .map_err(|err| format!("创建目录失败: {err}"))?;

    let client = reqwest::Client::new();
    let binary_bytes = download_binary_bytes(&client).await?;

    if binary_bytes.len() < 1024 {
        return Err("下载的 yt-dlp 文件异常（文件体积过小）".into());
    }

    verify_download_checksum(&client, &binary_bytes).await?;
    write_validated_binary(target_path, &binary_bytes).await
}

async fn download_binary_bytes(client: &reqwest::Client) -> Result<Vec<u8>, String> {
    let response = client
        .get(download_url())
        .send()
        .await
        .map_err(|err| format!("下载 yt-dlp 失败: {err}"))?;

    if !response.status().is_success() {
        return Err(format!("下载 yt-dlp 失败，状态码: {}", response.status()));
    }

    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|err| format!("读取下载内容失败: {err}"))
}

async fn verify_download_checksum(client: &reqwest::Client, bytes: &[u8]) -> Result<(), String> {
    let checksums = client
        .get(checksums_url())
        .send()
        .await
        .map_err(|err| format!("下载 yt-dlp 校验清单失败: {err}"))?;

    if !checksums.status().is_success() {
        return Err(format!(
            "下载 yt-dlp 校验清单失败，状态码: {}",
            checksums.status()
        ));
    }

    let body = checksums
        .bytes()
        .await
        .map_err(|err| format!("读取 yt-dlp 校验清单失败: {err}"))?;
    let body = String::from_utf8_lossy(&body);

    let expected = find_release_checksum(&body, release_asset_name()).ok_or_else(|| {
        format!(
            "未在 SHA2-256SUMS 中找到 {} 的校验值",
            release_asset_name()
        )
    })?;

    let actual = hex_sha256(bytes);
    if actual != expected {
        return Err("yt-dlp 下载校验失败，请稍后重试".into());
    }

    Ok(())
}

async fn write_validated_binary(target_path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp_path = temporary_binary_path(target_path);
    if temp_path.exists() {
        let _ = fs::remove_file(&temp_path).await;
    }

    if let Err(err) = fs::write(&temp_path, bytes).await {
        return Err(format!("写入 yt-dlp 文件失败: {err}"));
    }

    if let Err(err) = ensure_executable_permissions(&temp_path).await {
        let _ = fs::remove_file(&temp_path).await;
        return Err(err);
    }

    if let Err(err) = validate_binary(&temp_path) {
        let _ = fs::remove_file(&temp_path).await;
        return Err(format!("下载的 yt-dlp 无法运行: {err}"));
    }

    if target_path.exists() {
        let _ = fs::remove_file(target_path).await;
    }

    if let Err(err) = fs::rename(&temp_path, target_path).await {
        let _ = fs::remove_file(&temp_path).await;
        return Err(format!("替换 yt-dlp 文件失败: {err}"));
    }

    if let Err(err) = ensure_executable_permissions(target_path).await {
        let _ = fs::remove_file(target_path).await;
        return Err(err);
    }

    if let Err(err) = validate_binary(target_path) {
        let _ = fs::remove_file(target_path).await;
        return Err(format!("安装后的 yt-dlp 无法运行: {err}"));
    }

    if let Ok(mut cache) = capability_cache().lock() {
        cache.remove(&target_path.to_path_buf());
    }

    Ok(())
}

fn temporary_binary_path(target_path: &Path) -> PathBuf {
    let mut file_name = target_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("yt-dlp")
        .to_string();
    file_name.push_str(".download");

    target_path
        .parent()
        .map(|parent| parent.join(file_name))
        .unwrap_or_else(|| PathBuf::from(file_name))
}

fn validate_binary(path: &Path) -> Result<(), String> {
    get_version(path).map(|_| ())
}

fn find_release_checksum(content: &str, asset_name: &str) -> Option<String> {
    content.lines().find_map(|line| {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        let mut parts = trimmed.split_whitespace();
        let checksum = parts.next()?;
        let filename = parts.last()?;

        let normalized_name = filename
            .trim_start_matches('*')
            .trim_start_matches("./")
            .trim();

        if normalized_name != asset_name || checksum.len() != 64 {
            return None;
        }

        if checksum.chars().all(|char| char.is_ascii_hexdigit()) {
            Some(checksum.to_ascii_lowercase())
        } else {
            None
        }
    })
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|value| format!("{value:02x}")).collect()
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

#[cfg(test)]
mod tests {
    use super::find_release_checksum;

    #[test]
    fn parses_checksum_from_sums_file() {
        let sums = "abc123  yt-dlp\nffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff *yt-dlp.exe\n";

        let value = find_release_checksum(sums, "yt-dlp.exe").expect("checksum should be found");

        assert_eq!(
            value,
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
        );
    }

    #[test]
    fn ignores_invalid_checksum_rows() {
        let sums = "thisisnothex  yt-dlp\n";
        let value = find_release_checksum(sums, "yt-dlp");
        assert!(value.is_none());
    }
}
