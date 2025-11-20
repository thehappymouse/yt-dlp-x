# yt-dlp-x

一个基于 **Tauri 2** + **React** 的图形界面工具，封装 `yt-dlp`，为常见视频网站（尤其是 YouTube）提供便捷的音视频下载体验。

## 一次关于AI的尝试
- 使用cto.new工具，完成99%的逻辑代码
- Tauri 2的脚手架生成后，提交给cto，开始使用提示词生成代码
- 有意识的每次只提示一个功能，本地pull测试，避免一次新增过多功能而使其不能完成
- 手动修正的，如icon的类名，因为版本问题有变动；权限问题，交互了好几次才成功

## 功能亮点

- 自动检测系统中的 `yt-dlp`，若未安装可在线下载最新版本。
- 粘贴链接后，可一键选择下载 **最佳画质视频** 或 **纯音频 (MP3)**。
- 针对 YouTube 链接，默认启用 `--cookies-from-browser`，支持从常用浏览器读取登录态。
- 默认保存到系统下载目录，可自行修改并快速打开文件夹。
- 直观的执行日志，方便排查错误。

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

