import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Alert,
  Button,
  Card,
  ConfigProvider,
  Drawer,
  FloatButton,
  Form,
  Input,
  Progress,
  Radio,
  Select,
  Space,
  Tooltip,
  Typography,
} from "antd";
import {
  LuSquarePlay,
  LuFolder,
  LuFolderOpen,
  LuInfo,
  LuMenu,
  LuHeadphones,
  LuSettings,
  LuSquareChevronRight,
  LuHelpCircle,
} from "react-icons/lu";
import About from "./About";
import SettingsModal from "./SettingsModal";
import { APP_VERSION } from "./version";
import { extractErrorMessage } from "./utils/errors";
import "./App.css";
import logo from "./assets/logo.png";

const { Title, Text, Paragraph } = Typography;

const BROWSER_OPTIONS = [
  { label: "Chrome", value: "chrome" },
  { label: "Edge", value: "edge" },
  { label: "Firefox", value: "firefox" },
  { label: "Brave", value: "brave" },
  { label: "Safari (macOS)", value: "safari" },
];

const COOKIE_SOURCE_OPTIONS = [
  { label: "不使用 Cookie", value: "none" },
  ...BROWSER_OPTIONS,
];

const DEFAULT_BROWSER = "chrome";

const VIDEO_QUALITY_LABELS = {
  low: "低画质",
  medium: "中画质",
  highest: "最高画质",
};


function App() {
  const [url, setUrl] = useState("");
  const [browser, setBrowser] = useState(DEFAULT_BROWSER);
  const [downloadType, setDownloadType] = useState("video");
  const [videoQuality, setVideoQuality] = useState("highest");
  const [outputDir, setOutputDir] = useState("");
  const [isDownloading, setIsDownloading] = useState(false);
  const [downloadProgress, setDownloadProgress] = useState(null);
  const [logOutput, setLogOutput] = useState("");
  const [errorMessage, setErrorMessage] = useState("");
  const [successMessage, setSuccessMessage] = useState("");
  const [isAboutDialogOpen, setIsAboutDialogOpen] = useState(false);
  const [isSettingsModalOpen, setIsSettingsModalOpen] = useState(false);
  const [isLogDrawerOpen, setIsLogDrawerOpen] = useState(false);
  const [isParsing, setIsParsing] = useState(false);
  const [parseError, setParseError] = useState("");
  const [mediaPreview, setMediaPreview] = useState(null);
  const [lastParsedUrl, setLastParsedUrl] = useState("");

  const urlInputRef = useRef(null);
  const isParsingRef = useRef(false);
  const settingsModalRef = useRef(null);
  const [settingsStatusSnapshot, setSettingsStatusSnapshot] = useState(null);

  const hasPreview = Boolean(mediaPreview);

  const closeAboutDialog = () => setIsAboutDialogOpen(false);
  const openAboutDialog = () => setIsAboutDialogOpen(true);
  const closeSettingsModal = () => setIsSettingsModalOpen(false);
  const openSettingsModal = () => setIsSettingsModalOpen(true);
  const closeLogDrawer = () => setIsLogDrawerOpen(false);
  const openLogDrawer = () => setIsLogDrawerOpen(true);

  const activeSessionIdRef = useRef(null);
  const hasRealtimeLogsRef = useRef(false);
  const logContainerRef = useRef(null);
  const [isLogAutoScrollEnabled, setIsLogAutoScrollEnabled] = useState(true);

  useEffect(() => {
    urlInputRef.current?.focus();
  }, []);

  useEffect(() => {
    loadDefaultOutputDir();
  }, []);

  useEffect(() => {
    let unlistenLog;
    let unlistenProgress;

    listen("download-log", (event) => {
      const payload = event.payload;
      if (!payload || typeof payload !== "object") {
        return;
      }

      const { sessionId, line, stream } = payload;
      if (
        typeof sessionId !== "string" ||
        sessionId !== activeSessionIdRef.current
      ) {
        return;
      }
      if (typeof line !== "string") {
        return;
      }

      hasRealtimeLogsRef.current = true;

      const prefix = stream === "stderr" ? "[stderr] " : "";
      const formattedLine = prefix ? `${prefix}${line}` : line;

      setLogOutput((prev) =>
        prev ? `${prev}\n${formattedLine}` : formattedLine
      );
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

      const {
        sessionId,
        percent,
        percentText,
        eta,
        speed,
        total,
        status,
        raw,
      } = payload;
      if (
        typeof sessionId !== "string" ||
        sessionId !== activeSessionIdRef.current
      ) {
        return;
      }

      const parsedPercent =
        typeof percent === "number" && Number.isFinite(percent)
          ? Math.min(100, Math.max(0, percent))
          : null;
      const fallbackPercentText =
        parsedPercent !== null ? formatPercentText(parsedPercent) : null;

      setDownloadProgress((prev) => {
        const previous = prev ?? {
          percent: null,
          percentText: null,
          eta: null,
          speed: null,
          total: null,
          status: null,
          raw: null,
        };

        const nextPercent =
          parsedPercent !== null ? parsedPercent : previous.percent;
        const providedPercentText =
          typeof percentText === "string" && percentText.trim().length > 0
            ? percentText.trim()
            : null;

        return {
          percent: nextPercent,
          percentText:
            providedPercentText ??
            (parsedPercent !== null
              ? fallbackPercentText
              : previous.percentText),
          eta:
            typeof eta === "string" && eta.trim().length > 0
              ? eta.trim()
              : previous.eta,
          speed:
            typeof speed === "string" && speed.trim().length > 0
              ? speed.trim()
              : previous.speed,
          total:
            typeof total === "string" && total.trim().length > 0
              ? total.trim()
              : previous.total,
          status:
            typeof status === "string" && status.trim().length > 0
              ? status.trim()
              : previous.status,
          raw:
            typeof raw === "string" && raw.trim().length > 0
              ? raw.trim()
              : previous.raw,
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

  useEffect(() => {
    if (!isLogAutoScrollEnabled) {
      return;
    }

    const container = logContainerRef.current;
    if (!container) {
      return;
    }

    container.scrollTop = container.scrollHeight;
  }, [logOutput, isLogAutoScrollEnabled]);

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

  const handleUrlChange = (event) => {
    const nextValue = event.target.value;
    setUrl(nextValue);
    setErrorMessage("");
    setSuccessMessage("");
    if (parseError) {
      setParseError("");
    }
    const trimmedValue = nextValue.trim();
    if (mediaPreview && trimmedValue !== lastParsedUrl) {
      setMediaPreview(null);
      setLastParsedUrl("");
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

    if (!mediaPreview || trimmedUrl !== lastParsedUrl) {
      setErrorMessage("请先解析需要下载的视频链接。");
      setSuccessMessage("");
      return;
    }

    const browserForRequest = selectedBrowserOption?.value ?? null;

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
          browser: browserForRequest,
          outputDir,
          sessionId,
          quality: videoQuality,
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
      settingsModalRef.current?.refreshStatuses?.();
    }
  };

  const handleOpenDir = async () => {
    const targetDir = outputDir.trim();
    if (!targetDir) {
      return;
    }

    try {
      await invoke("open_directory", { path: targetDir });
    } catch (err) {
      setErrorMessage(`无法打开目录：${extractErrorMessage(err)}`);
    }
  };

  const handleSelectOutputDir = async () => {
    try {
      const trimmed = outputDir.trim();
      const selected = await open({
        directory: true,
        multiple: false,
        defaultPath: trimmed ? trimmed : undefined,
      });

      if (typeof selected === "string" && selected.trim()) {
        setOutputDir(selected);
      }
    } catch (err) {
      setErrorMessage(`选择下载目录失败：${extractErrorMessage(err)}`);
    }
  };

  const clearLog = () => {
    setLogOutput("");
  };

  const handleLogScroll = (event) => {
    const target = event.currentTarget;
    if (!target) {
      return;
    }

    const threshold = 12;
    const distanceToBottom =
      target.scrollHeight - target.scrollTop - target.clientHeight;
    const isAtBottom = distanceToBottom <= threshold;

    setIsLogAutoScrollEnabled((prev) =>
      prev === isAtBottom ? prev : isAtBottom
    );
  };

  const triggerParse = useCallback(
    async (force = false) => {
      const trimmedUrl = url.trim();
      if (!trimmedUrl) {
        if (force) {
          setParseError("请先输入需要解析的视频链接。");
          setMediaPreview(null);
          setLastParsedUrl("");
        }
        return;
      }

      if (!force && hasPreview && trimmedUrl === lastParsedUrl) {
        return;
      }

      if (isParsingRef.current) {
        return;
      }

      isParsingRef.current = true;
      setIsParsing(true);
      setParseError("");
      try {
        const response = await invoke("fetch_media_preview", {
          request: { url: trimmedUrl },
        });
        setMediaPreview({
          title: response?.title ?? "",
          thumbnail: response?.thumbnail ?? "",
          uploader: response?.uploader ?? "",
          duration:
            typeof response?.duration === "number" ? response.duration : null,
          extractor: response?.extractor ?? "",
          webpageUrl: response?.webpageUrl ?? "",
        });
        setLastParsedUrl(trimmedUrl);
        setParseError("");
      } catch (err) {
        setMediaPreview(null);
        setLastParsedUrl("");
        setParseError(`解析失败：${extractErrorMessage(err)}`);
      } finally {
        isParsingRef.current = false;
        setIsParsing(false);
      }
    },
    [url, hasPreview, lastParsedUrl]
  );

  const handleUrlBlur = useCallback(() => {
    triggerParse(false);
  }, [triggerParse]);

  const handleUrlPressEnter = useCallback(
    (event) => {
      if (!hasPreview) {
        event.preventDefault();
        triggerParse(true);
      }
    },
    [hasPreview, triggerParse]
  );

  const handleVideoQualityUpdate = useCallback(
    (nextQuality) => {
      if (typeof nextQuality !== "string") {
        return;
      }

      if (!VIDEO_QUALITY_LABELS[nextQuality]) {
        return;
      }

      setVideoQuality(nextQuality);
    },
    [setVideoQuality]
  );

  const handleSettingsStatusChange = useCallback((nextStatus) => {
    setSettingsStatusSnapshot((prev) => {
      if (!nextStatus) {
        return null;
      }

      if (
        prev &&
        prev.ytInstalled === nextStatus.ytInstalled &&
        prev.ffInstalled === nextStatus.ffInstalled &&
        prev.checkingYt === nextStatus.checkingYt &&
        prev.checkingFf === nextStatus.checkingFf
      ) {
        return prev;
      }

      return nextStatus;
    });
  }, []);

  const selectedBrowserOption = useMemo(
    () => BROWSER_OPTIONS.find((option) => option.value === browser) ?? null,
    [browser]
  );

  const isUsingBrowserCookies = Boolean(selectedBrowserOption);

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
      typeof downloadProgress.percent === "number" &&
      Number.isFinite(downloadProgress.percent);
    const percentValue = hasPercent
      ? Math.min(100, Math.max(0, downloadProgress.percent))
      : null;

    const percentLabel =
      typeof downloadProgress.percentText === "string" &&
      downloadProgress.percentText
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

    const parts = [
      percentLabel,
      downloadProgress.total,
      downloadProgress.speed,
      etaLabel,
    ].filter((value) => typeof value === "string" && value.length > 0);

    if (parts.length > 0) {
      return parts.join(" · ");
    }

    if (downloadProgress.raw) {
      return downloadProgress.raw;
    }

    return "准备中...";
  }, [downloadProgress]);

  const previewDurationLabel = useMemo(() => {
    if (!mediaPreview || typeof mediaPreview.duration !== "number") {
      return null;
    }

    return formatDuration(mediaPreview.duration);
  }, [mediaPreview]);

  const showYtDlpWarningBadge = useMemo(() => {
    if (!settingsStatusSnapshot) {
      return false;
    }

    if (settingsStatusSnapshot.checkingYt) {
      return false;
    }

    return !settingsStatusSnapshot.ytInstalled;
  }, [settingsStatusSnapshot]);

  const cookieTooltipMessage =
    "选择浏览器后，工具会尝试读取该浏览器的 cookies 支持所有站点的下载（需浏览器已登录），也可以选择“不使用 Cookie”。";

  const cookieLabelNode = (
    <span className="form-label-with-help">
      Cookies 浏览器
      <Tooltip title={cookieTooltipMessage}>
        <span className="form-label-help-icon" role="img" aria-label="Cookie 提示">
          <LuHelpCircle size={16} strokeWidth={2.5} />
        </span>
      </Tooltip>
    </span>
  );

  const appShellClassName = ["app-shell", hasPreview ? "" : "hero-mode"]
    .filter(Boolean)
    .join(" ");

  const downloadCardClassName = [
    "download-card",
    hasPreview ? "download-card-active" : "download-card-hero",
  ].join(" ");

  return (
    <ConfigProvider
      theme={{
        token: {
          colorPrimary: "#2563eb",
          borderRadius: 2,
          borderRadiusLG: 2,
          borderRadiusSM: 2,
          borderRadiusXS: 2,
          borderRadiusOuter: 2,
        },
        components: {
          Segmented: {
            controlHeight: 42,
          },
        },
      }}
    >
      <div className="app-background">
        <div className={appShellClassName}>
          <div className="main-stack">
            <Card border={false} className="hero-card">
              <Space direction="vertical" size="middle" align="center">
                <Space align="center" size="middle" wrap>
                  <img src={logo} alt="yt-dlp-x logo" className="app-logo" />
                  <div>
                    <Title level={2} style={{ margin: 0, textAlign: "left" }}>
                      yt-dlp-x
                    </Title>
                    <Text type="secondary">
                      一款基于 Tauri2 的 yt-dlp 图形化下载工具
                    </Text>
                  </div>
                </Space>
              </Space>
            </Card>

            <Card className={downloadCardClassName}>
              <Form layout="vertical" onSubmitCapture={handleDownload}>
                <Form.Item
                  label={hasPreview ? "视频链接" : null}
                  required
                  colon={false}
                  className={`url-form-item ${
                    hasPreview ? "url-form-item-compact" : "url-form-item-hero"
                  }`}
                >
                  <Space.Compact
                    style={{ width: "100%" }}
                    className={`url-input-group ${
                      hasPreview ? "" : "url-input-group-hero"
                    }`}
                  >
                    <Input
                      ref={urlInputRef}
                      value={url}
                      onChange={handleUrlChange}
                      onBlur={handleUrlBlur}
                      onPressEnter={handleUrlPressEnter}
                      placeholder="粘贴 YouTube 或其它站点的链接"
                      size="large"
                      allowClear
                      className={`url-input ${
                        hasPreview ? "" : "url-input-hero"
                      }`}
                    />
                    <Button
                      type="primary"
                      size="large"
                      className="parse-button"
                      onClick={() => triggerParse(true)}
                      loading={isParsing}
                    >
                      解析
                    </Button>
                  </Space.Compact>
                </Form.Item>

                {parseError && (
                  <Alert type="error" showIcon message={parseError} />
                )}

                {mediaPreview && (
                  <div className="preview-panel">
                    <div className="preview-cover">
                      {mediaPreview.thumbnail ? (
                        <img
                          src={mediaPreview.thumbnail}
                          alt={mediaPreview.title || "视频封面"}
                        />
                      ) : (
                        <div className="preview-placeholder">暂无封面</div>
                      )}
                    </div>
                    <div className="preview-meta">
                      <Title level={4} className="preview-title">
                        {mediaPreview.title || "未获取到标题"}
                      </Title>
                      <Space size="small" wrap>
                        {mediaPreview.uploader ? (
                          <Text>{mediaPreview.uploader}</Text>
                        ) : null}
                        {previewDurationLabel ? (
                          <Text type="secondary">
                            时长 {previewDurationLabel}
                          </Text>
                        ) : null}
                        {mediaPreview.extractor ? (
                          <Text type="secondary">{mediaPreview.extractor}</Text>
                        ) : null}
                      </Space>
                    </div>
                  </div>
                )}

                {mediaPreview ? (
                  <div className="download-options">
                    <Form.Item label="下载类型" colon={false}>
                      <Radio.Group
                        value={downloadType}
                        onChange={(event) => setDownloadType(event.target.value)}
                        disabled={isDownloading}
                        className="download-type-radios"
                      >
                        <Space direction="vertical" size="middle">
                          <Radio value="video">
                            <Space size={8}>
                              <LuSquarePlay size={18} strokeWidth={2} />
                              <span>
                                视频（
                                {VIDEO_QUALITY_LABELS[videoQuality] ?? "最高画质"}
                                ）
                              </span>
                            </Space>
                          </Radio>
                          <Radio value="audio">
                            <Space size={8}>
                              <LuHeadphones size={18} strokeWidth={2} />
                              <span>纯音频 (MP3)</span>
                            </Space>
                          </Radio>
                        </Space>
                      </Radio.Group>
                    </Form.Item>

                    <Form.Item label={cookieLabelNode} colon={false}>
                      <Select
                        value={browser}
                        onChange={(value) => setBrowser(value)}
                        disabled={isDownloading}
                        options={COOKIE_SOURCE_OPTIONS}
                      />
                    </Form.Item>

                    <Form.Item label="保存位置" colon={false}>
                      <Space.Compact style={{ width: "100%" }}>
                        <Input
                          value={outputDir}
                          onChange={(event) => setOutputDir(event.target.value)}
                          placeholder="下载保存目录"
                        />
                        <Button
                          icon={<LuFolder size={18} strokeWidth={1.75} />}
                          onClick={handleSelectOutputDir}
                        >
                          更换
                        </Button>
                        <Button
                          icon={<LuFolderOpen size={18} strokeWidth={1.75} />}
                          onClick={handleOpenDir}
                          type="link"
                          disabled={!outputDir.trim()}
                        >
                          打开
                        </Button>
                      </Space.Compact>
                    </Form.Item>

                    {(isUsingBrowserCookies || errorMessage || successMessage) && (
                      <Space direction="vertical" style={{ width: "100%" }}>
                        {isUsingBrowserCookies && selectedBrowserOption && (
                          <Alert
                            type="info"
                            showIcon
                            message={`工具会尝试使用 ${selectedBrowserOption.label} 浏览器的 cookies 支持所有站点的下载（请确保该浏览器已登录）。`}
                          />
                        )}
                        {errorMessage && (
                          <Alert type="error" showIcon message={errorMessage} />
                        )}
                        {successMessage && (
                          <Alert type="success" showIcon message={successMessage} />
                        )}
                      </Space>
                    )}

                    <Form.Item>
                      <Space direction="vertical" style={{ width: "100%" }}>
                        <Button
                          type="primary"
                          htmlType="submit"
                          block
                          size="large"
                          style={{ marginTop: 5 }}
                          loading={isDownloading}
                          disabled={!url.trim()}
                        >
                          {downloadButtonLabel}
                        </Button>
                        {isDownloading && (
                          <Space
                            direction="vertical"
                            size={4}
                            style={{ width: "100%" }}
                          >
                            <Progress
                              percent={progressPercent ?? 0}
                              status={
                                progressPercent === null ? "active" : undefined
                              }
                              showInfo={false}
                            />
                            <Text type="secondary">{progressText}</Text>
                          </Space>
                        )}
                      </Space>
                    </Form.Item>
                  </div>
                ) : null}
              </Form>
            </Card>
          </div>
        </div>
      </div>

      <FloatButton.Group
        trigger="click"
        type="primary"
        icon={<LuMenu size={20} strokeWidth={1.75} />}
        shape="square"
        badge={
          showYtDlpWarningBadge ? { dot: true, color: "#ff4d4f" } : undefined
        }
        style={{ right: 24, bottom: 24 }}
      >
        <FloatButton
          icon={<LuInfo size={20} strokeWidth={1.75} />}
          onClick={openAboutDialog}
        />
        <FloatButton
          icon={<LuSettings size={20} strokeWidth={1.75} />}
          onClick={openSettingsModal}
          badge={
            showYtDlpWarningBadge ? { dot: true, color: "#ff4d4f" } : undefined
          }
        />
        <FloatButton
          icon={<LuSquareChevronRight size={20} strokeWidth={1.75} />}
          onClick={openLogDrawer}
        />
      </FloatButton.Group>

      <SettingsModal
        ref={settingsModalRef}
        open={isSettingsModalOpen}
        onClose={closeSettingsModal}
        isDownloading={isDownloading}
        onStatusChange={handleSettingsStatusChange}
        videoQuality={videoQuality}
        onVideoQualityChange={handleVideoQualityUpdate}
      />

      <Drawer
        title="Debugger"
        placement="bottom"
        height={450}
        onClose={closeLogDrawer}
        open={isLogDrawerOpen}
        styles={{
          body: { padding: 10 },
        }}
        extra={
          <Button onClick={clearLog} disabled={!logOutput}>
            清空
          </Button>
        }
      >
        <Paragraph
          className="log-output"
          ref={logContainerRef}
          onScroll={handleLogScroll}
        >
          {logOutput || "暂无输出"}
        </Paragraph>
      </Drawer>

      <About open={isAboutDialogOpen} onClose={closeAboutDialog} />
    </ConfigProvider>
  );
}

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

const formatDuration = (value) => {
  if (typeof value !== "number" || !Number.isFinite(value) || value <= 0) {
    return null;
  }

  const totalSeconds = Math.max(0, Math.floor(value));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  const pad = (unit) => (unit < 10 ? `0${unit}` : `${unit}`);

  if (hours > 0) {
    return `${hours}:${pad(minutes)}:${pad(seconds)}`;
  }

  return `${minutes}:${pad(seconds)}`;
};

export default App;
