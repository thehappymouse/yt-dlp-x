// src-tauri/src/utils/yt_dlp.rs

use std::path::PathBuf;
use std::process::Command;
use tokio::fs;
use tokio::process::Command as TokioCommand;

pub fn get_yt_dlp_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    let file_name = "yt-dlp.exe";
    #[cfg(not(target_os = "windows"))]
    let file_name = "yt-dlp";

    let home_dir = dirs::home_dir().expect("无法获取用户主目录");
    home_dir.join(".yt-dlp-x").join(file_name)
}

pub async fn ensure_yt_dlp_installed() -> Result<PathBuf, String> {
    let bundled_path = get_yt_dlp_path();

    // 先检查系统 PATH 中是否有 yt-dlp
    if Command::new("yt-dlp").arg("--version").output().is_ok() {
        return Ok(PathBuf::from("yt-dlp"));
    }

    // 再检查本地是否已下载
    if bundled_path.exists() {
        #[cfg(not(target_os = "windows"))]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&bundled_path).await.unwrap().permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(&bundled_path, perms).await;
        }
        return Ok(bundled_path);
    }

    // 否则下载
    download_yt_dlp(&bundled_path).await?;
    Ok(bundled_path)
}

async fn download_yt_dlp(target_path: &PathBuf) -> Result<(), String> {
    let download_url = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp";

    let client = reqwest::Client::new();
    let resp = client
        .get(download_url)
        .send()
        .await
        .map_err(|e| format!("下载失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP 错误: {}", resp.status()));
    }

    let data = resp
        .bytes()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;

    fs::create_dir_all(target_path.parent().unwrap())
        .await
        .map_err(|e| format!("创建目录失败: {}", e))?;

    fs::write(target_path, data)
        .await
        .map_err(|e| format!("写入文件失败: {}", e))?;

    #[cfg(not(target_os = "windows"))]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(target_path).await.unwrap().permissions();
        perms.set_mode(0o755);
        let _ = fs::set_permissions(target_path, perms).await;
    }

    Ok(())
}