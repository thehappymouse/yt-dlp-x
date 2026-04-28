# yt-dlp-x

一个基于 **Tauri 2** + **React** 的图形界面工具，封装 `yt-dlp`，为常见视频网站（尤其是 YouTube）提供便捷的音视频下载体验。

## 一次关于AI的尝试
- 使用cto.new工具，完成99%的逻辑代码
- Tauri 2的脚手架生成后，提交给cto，开始使用提示词生成代码
- 有意识的每次只提示一个功能，本地pull测试，避免一次新增过多功能而使其不能完成
- 手动修正的，如icon的类名，因为版本问题有变动；权限问题，交互了好几次才成功

## 功能亮点

- 自动检测系统中的 `yt-dlp`，若未安装可在线下载最新版本。
- `Settings` 中可查看当前 `yt-dlp` 的来源（system / bundled）与版本号。
- 粘贴链接后，可一键选择下载 **最佳画质视频** 或 **纯音频 (MP3)**。
- 支持高级下载参数（文件名模板、重试、分片重试、文件访问重试、并发分片、重试间隔）。
- 默认保存到系统下载目录，可自行修改并快速打开文件夹。
- 直观的执行日志，方便排查错误。

## yt-dlp 参数与兼容策略

### 参数分层

当前后端会按以下层级构造参数：

1. **默认参数**：`--newline`、`--no-playlist`、`--continue`、`--no-mtime` 等。
2. **模式参数**：
   - 音频模式：`bestaudio/best` + `-x --audio-format mp3` + 封面处理。
   - 视频模式：根据画质模板选择 format，并默认 `--merge-output-format mp4`。
3. **可选参数**：`--cookies-from-browser`、`--ffmpeg-location`、重试/并发参数。
4. **站点覆盖**：抖音链接优先使用 `--add-headers` 注入 `Referer` 和 `User-Agent`。

### 兼容策略

- 针对较新版本 `yt-dlp`（当前目标：2026.03.17），优先使用：
  - `--add-headers "Referer: ..."`
  - `--add-headers "User-Agent: ..."`
- 如果检测到运行时不支持 `--add-headers`，自动回退到旧参数（`--referer` / `--user-agent`）。
- 进度输出优先使用 `--progress-template` 生成稳定可解析行；若不可用则回退旧的 `[download] ...` 解析逻辑。
- `--paths` 支持时自动启用 `home` 与 `temp` 分离（临时目录默认 `下载目录/.yt-dlp-temp`）。

### 下载稳定性默认值

- `-R/--retries`: `10`
- `--fragment-retries`: `10`
- `--file-access-retries`: `3`
- `--retry-sleep`: `1`
- `-N/--concurrent-fragments`: 默认 `1`（仅当 >1 时显式写入）

## yt-dlp 安装与校验

- 自动安装时会下载官方发布二进制，并读取 `SHA2-256SUMS` 进行 SHA-256 校验。
- 下载后会执行 `yt-dlp --version` 做可执行性校验。
- 校验失败会清理临时/损坏文件，避免后续命中坏 binary。

## 开发指引

```bash
# 安装依赖
$ yarn install

# 开发模式（会同时启动前端与 Tauri）
$ yarn tauri dev

# 生产打包
$ yarn tauri build
```

> 首次运行若缺少 `yt-dlp`，应用会自动下载对应平台的可执行文件并保存到应用数据目录。

## 回归测试

```bash
# 运行 Rust 单元测试（参数构造、进度解析、校验解析）
cd src-tauri && cargo test
```

## 发布构建

Tauri 构建产物默认输出到 `src-tauri/target/release/bundle` 目录。可直接执行 `yarn release` 生成当前平台的默认产物；下列命令则需在目标平台执行，或在配置好交叉编译工具链的环境中运行，以构建特定安装包：

### Windows

```bash
yarn release:windows
```

会生成 `.msi` 安装包与 `.nsis.zip` 安装器，可直接分发。

### macOS

```bash
# Apple Silicon
yarn release:macos

# 如需打包通用二进制，请在安装额外 target 后执行
# yarn release:macos:universal
```

默认输出 `.app` 与 `.dmg`。由于尚未进行开发者签名，首次运行可能会提示应用已损坏，可执行：

```bash
xattr -cr "/Applications/yt-dlp-x.app"
```

将路径替换为实际存放位置，以移除隔离属性后即可正常启动。

### Linux

```bash
yarn release:linux
```

会生成 `.AppImage`、`.deb` 与 `.rpm` 等发行包。

## 目录结构

- `src/` — React 前端界面
- `src-tauri/` — Tauri 2 Rust 主程序、命令处理与 `yt-dlp` 管理逻辑

## 许可证

本项目使用 [The Unlicense](./LICENSE) 授权，保持与 yt-dlp 相同。

## 发布列表：
### Release（2025-11-07）已满足基本工具需要

## 后续开发计划

- 界面整洁，突出重点
- 下载历史（本地json文件），可清空
- 支持yt的播放列表下载
- 抖音中国（打开浏览器图形下载）

