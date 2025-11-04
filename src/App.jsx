import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openPath } from "@tauri-apps/plugin-opener";
import "./App.css";

const BROWSER_OPTIONS = [
  { label: "自动选择 (Chrome / Edge / Firefox / Brave)", value: "chrome,edge,firefox,brave" },
  { label: "Chrome", value: "chrome" },
  { label: "Edge", value: "edge" },
  { label: "Firefox", value: "firefox" },
  { label: "Brave", value: "brave" },
  { label: "Safari (macOS)", value: "safari" },
];

const DEFAULT_BROWSER = BROWSER_OPTIONS[0].value;

const extractErrorMessage = (error) => {
  if (!error) return "未知错误";
  if (typeof error === "string") return error;
  if (typeof error === "object") {
    if ("message" in error && error.message) {
      return error.message;
    }
    try {
      return JSON.stringify(error);
    } catch (serializationError) {
      return String(error);
    }
  }

  return String(error);
};

function App() {
  const [url, setUrl] = useState("");
  const [browser, setBrowser] = useState(DEFAULT_BROWSER);
  const [downloadType, setDownloadType] = useState("video");
  const [outputDir, setOutputDir] = useState("");
  const [ytStatus, setYtStatus] = useState({ installed: false, path: "", source: "" });
  const [checkingYt, setCheckingYt] = useState(true);
  const [installing, setInstalling] = useState(false);
  const [isDownloading, setIsDownloading] = useState(false);
  const [downloadProgress, setDownloadProgress] = useState(null);
  const [logOutput, setLogOutput] = useState("");
  const [errorMessage, setErrorMessage] = useState("");
  const [successMessage, setSuccessMessage] = useState("");

  const activeSessionIdRef = useRef(null);
  const hasRealtimeLogsRef = useRef(false);

  useEffect(() => {
    refreshYtStatus();
    loadDefaultOutputDir();
  }, []);

  const formatPercentText = (value) => {
    if (typeof value !== "number" || !Number.isFinite(value)) {
      return null;
    }

    const clamped = Math.min(100, Math.max(0, value));
    if (clamped === 100) {
      return "100%";
    }

    const floored = Math.floor(clamped * 10) / 10;
    const hasFraction = Math.abs(floored - Math.trunc(floored)) > Number.EPSILON;

    if (hasFraction) {
      return `${floored.toFixed(1)}%`;
    }

    return `${Math.trunc(floored)}%`;
  };

  useEffect(() => {
    let unlistenLog;
    let unlistenProgress;

    listen("download-log", (event) => {
      const payload = event.payload;
      if (!payload || typeof payload !== "object") {
        return;
      }

      const { sessionId, line, stream } = payload;
      if (typeof sessionId !== "string" || sessionId !== activeSessionIdRef.current) {
        return;
      }
      if (typeof line !== "string") {
        return;
      }

      hasRealtimeLogsRef.current = true;

      const prefix = stream === "stderr" ? "[stderr] " : "";
      const formattedLine = prefix ? `${prefix}${line}` : line;

      setLogOutput((prev) => (prev ? `${prev}\n${formattedLine}` : formattedLine));
    })
      .then((unlisten) => {
        unlistenLog = unlisten;
      })
      .catch((err) => {
        console.error("监听日志事件失败", err);
      });

    listen("download-progress", (event) => {
      const payload = event.payload;
      if (!payload || typeof payload !== "object") {
        return;
      }

      const { sessionId, percent, percentText, eta, speed, total, status, raw } = payload;
      if (typeof sessionId !== "string" || sessionId !== activeSessionIdRef.current) {
        return;
      }

      const parsedPercent =
        typeof percent === "number" && Number.isFinite(percent) ? Math.min(100, Math.max(0, percent)) : null;
      const fallbackPercentText = parsedPercent !== null ? formatPercentText(parsedPercent) : null;

      setDownloadProgress((prev) => {
        const previous =
          prev ?? {
            percent: null,
            percentText: null,
            eta: null,
            speed: null,
            total: null,
            status: null,
            raw: null,
          };

        const nextPercent = parsedPercent !== null ? parsedPercent : previous.percent;
        const providedPercentText =
          typeof percentText === "string" && percentText.trim().length > 0 ? percentText.trim() : null;

        return {
          percent: nextPercent,
          percentText: providedPercentText ?? (parsedPercent !== null ? fallbackPercentText : previous.percentText),
          eta: typeof eta === "string" && eta.trim().length > 0 ? eta.trim() : previous.eta,
          speed: typeof speed === "string" && speed.trim().length > 0 ? speed.trim() : previous.speed,
          total: typeof total === "string" && total.trim().length > 0 ? total.trim() : previous.total,
          status: typeof status === "string" && status.trim().length > 0 ? status.trim() : previous.status,
          raw: typeof raw === "string" && raw.trim().length > 0 ? raw.trim() : previous.raw,
        };
      });
    })
      .then((unlisten) => {
        unlistenProgress = unlisten;
      })
      .catch((err) => {
        console.error("监听进度事件失败", err);
      });

    return () => {
      if (unlistenLog) {
        unlistenLog();
      }
      if (unlistenProgress) {
        unlistenProgress();
      }
    };
  }, []);

  const refreshYtStatus = async () => {
    try {
      setCheckingYt(true);
      const status = await invoke("check_yt_dlp");
      setYtStatus({
        installed: Boolean(status.installed),
        path: status.path ?? "",
        source: status.source ?? "",
      });
    } catch (err) {
      setErrorMessage(`检测 yt-dlp 失败：${extractErrorMessage(err)}`);
    } finally {
      setCheckingYt(false);
    }
  };

  const loadDefaultOutputDir = async () => {
    try {
      const dir = await invoke("get_default_download_dir");
      if (typeof dir === "string") {
        setOutputDir(dir);
      }
    } catch (err) {
      console.warn("获取默认下载目录失败", err);
    }
  };

  const installYtDlp = async () => {
    setInstalling(true);
    setErrorMessage("");
    setSuccessMessage("");
    try {
      const status = await invoke("install_yt_dlp");
      setYtStatus({
        installed: Boolean(status.installed),
        path: status.path ?? "",
        source: status.source ?? "",
      });
      setSuccessMessage("yt-dlp 已下载并可用。");
    } catch (err) {
      setErrorMessage(`安装 yt-dlp 失败：${extractErrorMessage(err)}`);
    } finally {
      setInstalling(false);
    }
  };

  const handleDownload = async (event) => {
    event.preventDefault();
    const trimmedUrl = url.trim();
    if (!trimmedUrl) {
      setErrorMessage("请先输入需要下载的视频链接。");
      setSuccessMessage("");
      return;
    }

    const sessionId =
      typeof globalThis !== "undefined" &&
      globalThis.crypto &&
      typeof globalThis.crypto.randomUUID === "function"
        ? globalThis.crypto.randomUUID()
        : `${Date.now()}`;

    activeSessionIdRef.current = sessionId;
    hasRealtimeLogsRef.current = false;

    setIsDownloading(true);
    setDownloadProgress({
      percent: 0,
      percentText: "0%",
      eta: null,
      speed: null,
      total: null,
      status: "pending",
      raw: null,
    });
    setErrorMessage("");
    setSuccessMessage("");
    setLogOutput("");

    try {
      const response = await invoke("download_media", {
        request: {
          url: trimmedUrl,
          mode: downloadType,
          browser,
          outputDir,
          sessionId,
        },
      });

      const stdout = typeof response.stdout === "string" ? response.stdout : "";
      const stderr = typeof response.stderr === "string" ? response.stderr : "";
      const combined = [stdout, stderr].filter(Boolean).join("\n\n");

      if (!hasRealtimeLogsRef.current) {
        setLogOutput(combined || "命令执行完成。");
      } else {
        setLogOutput((prev) => prev || "命令执行完成。");
      }

      if (response.success) {
        const targetDir = response.outputDir || outputDir;
        setSuccessMessage(`下载完成，文件保存于：${targetDir}`);
      } else {
        setErrorMessage(stderr || "下载失败，请查看日志输出。");
      }
    } catch (err) {
      setErrorMessage(`下载失败：${extractErrorMessage(err)}`);
    } finally {
      setIsDownloading(false);
      setDownloadProgress(null);
      refreshYtStatus();
    }
  };

  const handleOpenDir = async () => {
    const targetDir = outputDir.trim();
    if (!targetDir) {
      return;
    }

    try {
      await openPath(targetDir);
    } catch (err) {
      setErrorMessage(`无法打开目录：${extractErrorMessage(err)}`);
    }
  };

  const clearLog = () => {
    setLogOutput("");
  };

  const isYoutubeUrl = useMemo(() => {
    const value = url.trim().toLowerCase();
    return value.includes("youtube.com") || value.includes("youtu.be");
  }, [url]);

  const downloadButtonLabel = useMemo(() => {
    if (isDownloading) {
      return "正在下载...";
    }

    return downloadType === "video" ? "下载视频" : "下载音频";
  }, [downloadType, isDownloading]);

  const progressPercent = useMemo(() => {
    if (
      !downloadProgress ||
      typeof downloadProgress.percent !== "number" ||
      !Number.isFinite(downloadProgress.percent)
    ) {
      return downloadProgress?.status === "pending" ? 0 : null;
    }

    return Math.min(100, Math.max(0, downloadProgress.percent));
  }, [downloadProgress]);

  const progressText = useMemo(() => {
    if (!downloadProgress) {
      return "准备中...";
    }

    if (downloadProgress.status === "pending") {
      return "准备中...";
    }

    const hasPercent =
      typeof downloadProgress.percent === "number" && Number.isFinite(downloadProgress.percent);
    const percentValue = hasPercent ? Math.min(100, Math.max(0, downloadProgress.percent)) : null;

    const percentLabel =
      typeof downloadProgress.percentText === "string" && downloadProgress.percentText
        ? downloadProgress.percentText
        : percentValue !== null
        ? formatPercentText(percentValue)
        : null;

    const etaLabel =
      downloadProgress.eta && downloadProgress.status === "downloading"
        ? `剩余 ${downloadProgress.eta}`
        : downloadProgress.eta && downloadProgress.status === "finished"
        ? `耗时 ${downloadProgress.eta}`
        : null;

    const parts = [percentLabel, downloadProgress.total, downloadProgress.speed, etaLabel].filter(
      (value) => typeof value === "string" && value.length > 0,
    );

    if (parts.length > 0) {
      return parts.join(" · ");
    }

    if (downloadProgress.raw) {
      return downloadProgress.raw;
    }

    return "准备中...";
  }, [downloadProgress]);

  const progressBarWidth = progressPercent ?? (downloadProgress ? 5 : 0);

  const ytStatusLabel = checkingYt
    ? "正在检测 yt-dlp..."
    : ytStatus.installed
    ? `yt-dlp 已就绪（${ytStatus.source === "system" ? "系统版本" : "内置版本"}）`
    : "尚未检测到 yt-dlp";

  return (
    <div className="app-container">
      <header>
        <h1>yt-dlp-x</h1>
        <p>基于 Tauri 2 的 yt-dlp 图形界面，支持音视频分离下载。</p>
      </header>

      <section className="card status-card">
        <div className="status-row">
          <div
            className={[
              "status-pill",
              checkingYt ? "loading" : ytStatus.installed ? "success" : "warning",
            ].join(" ")}
          >
            {ytStatusLabel}
          </div>
          <div className="status-actions">
            <button
              type="button"
              className="outline"
              onClick={refreshYtStatus}
              disabled={checkingYt || isDownloading}
            >
              重新检测
            </button>
            <button
              type="button"
              className="outline"
              onClick={installYtDlp}
              disabled={installing}
            >
              {installing ? "正在安装..." : "安装 / 更新 yt-dlp"}
            </button>
          </div>
        </div>
        {ytStatus.path ? (
          <p className="status-path">当前使用的 yt-dlp 路径：{ytStatus.path}</p>
        ) : (
          <p className="status-path">将自动在首次下载时获取 yt-dlp。</p>
        )}
      </section>

      <form className="card download-card" onSubmit={handleDownload}>
        <div className="form-group">
          <label htmlFor="url">视频链接</label>
          <input
            id="url"
            type="text"
            placeholder="粘贴 YouTube 或其它站点的链接"
            value={url}
            onChange={(event) => setUrl(event.target.value)}
          />
        </div>

        <div className="form-group">
          <label>下载类型</label>
          <div className="mode-toggle">
            <button
              type="button"
              className={downloadType === "video" ? "active" : ""}
              onClick={() => setDownloadType("video")}
              disabled={isDownloading}
            >
              视频 (最佳画质)
            </button>
            <button
              type="button"
              className={downloadType === "audio" ? "active" : ""}
              onClick={() => setDownloadType("audio")}
              disabled={isDownloading}
            >
              纯音频 (MP3)
            </button>
          </div>
        </div>

        <div className="form-group">
          <label htmlFor="browser">YouTube Cookies 浏览器</label>
          <select
            id="browser"
            value={browser}
            onChange={(event) => setBrowser(event.target.value)}
            disabled={isDownloading}
          >
            {BROWSER_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
          <span className="field-helper">
            下载 YouTube 视频时，会尝试自动从所选浏览器读取 cookies（需浏览器已登录）。
          </span>
        </div>

        <div className="form-group">
          <label htmlFor="outputDir">保存位置</label>
          <div className="output-row">
            <input
              id="outputDir"
              type="text"
              value={outputDir}
              onChange={(event) => setOutputDir(event.target.value)}
              placeholder="下载保存目录"
            />
            <button
              type="button"
              className="outline"
              onClick={handleOpenDir}
              disabled={!outputDir.trim()}
            >
              打开
            </button>
          </div>
          <span className="field-helper">默认使用系统的下载目录，你可以自行修改。</span>
        </div>

        {isYoutubeUrl && (
          <div className="alert info">
            已检测到 YouTube 链接，将按所选浏览器自动附加 cookies。
          </div>
        )}

        {errorMessage && <div className="alert error">{errorMessage}</div>}
        {successMessage && <div className="alert success">{successMessage}</div>}

        <div className="form-actions">
          <div className="action-wrapper">
            <button
              type="submit"
              className="primary"
              disabled={isDownloading || !url.trim()}
            >
              {downloadButtonLabel}
            </button>
            {isDownloading && (
              <div className="download-progress">
                <div className="download-progress-track">
                  <div
                    className="download-progress-bar"
                    style={{ width: `${Math.max(0, Math.min(100, progressBarWidth))}%` }}
                  />
                </div>
                <div className="download-progress-text">{progressText}</div>
              </div>
            )}
          </div>
        </div>
      </form>

      <section className="card log-card">
        <div className="log-header">
          <h2>日志输出</h2>
          <button
            type="button"
            className="outline"
            onClick={clearLog}
            disabled={!logOutput}
          >
            清空
          </button>
        </div>
        <pre className="log-output">{logOutput || "暂无输出"}</pre>
      </section>
    </div>
  );
}

export default App;
