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
  Empty,
  FloatButton,
  Form,
  Input,
  Progress,
  Radio,
  Select,
  Skeleton,
  Space,
  Tag,
  Tooltip,
  Typography,
} from "antd";
import { PictureOutlined, QuestionCircleOutlined } from "@ant-design/icons";
import {
  LuSquarePlay,
  LuFolder,
  LuFolderOpen,
  LuInfo,
  LuMenu,
  LuHeadphones,
  LuSettings,
  LuSquareChevronRight,
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

const COOKIE_TIP =
  "选择浏览器后，工具会尝试读取该浏览器的 cookies 支持所有站点的下载（需浏览器已登录），也可以选择“不使用 Cookie”。";

const DEFAULT_BROWSER = "chrome";

const VIDEO_QUALITY_LABELS = {
  low: "低画质",
  medium: "中画质",
  highest: "最高画质",
};


function App() {
  const [form] = Form.useForm();
  const currentUrl = Form.useWatch("url", form) ?? "";
  const normalizedUrl = currentUrl.trim();
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
  const [metadataPreview, setMetadataPreview] = useState({
    status: "idle",
    data: null,
    error: "",
    sourceUrl: "",
  });

  const urlInputRef = useRef(null);
  const settingsModalRef = useRef(null);
  const [settingsStatusSnapshot, setSettingsStatusSnapshot] = useState(null);

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
    setMetadataPreview((prev) => {
      if (!normalizedUrl) {
        if (
          prev.status === "idle" &&
          !prev.data &&
          !prev.error &&
          !prev.sourceUrl
        ) {
          return prev;
        }

        return {
          status: "idle",
          data: null,
          error: "",
          sourceUrl: "",
        };
      }

      if (prev.sourceUrl === normalizedUrl || prev.status === "loading") {
        return prev;
      }

      if (prev.status === "idle") {
        return prev;
      }

      return {
        status: "idle",
        data: null,
        error: "",
        sourceUrl: "",
      };
    });
  }, [normalizedUrl]);

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

  const isUrlFormatValid = useMemo(() => isLikelyUrl(normalizedUrl), [normalizedUrl]);

  const metadataLoading =
    metadataPreview.status === "loading" &&
    metadataPreview.sourceUrl === normalizedUrl &&
    Boolean(normalizedUrl);

  const metadataResolved =
    metadataPreview.status === "success" &&
    metadataPreview.sourceUrl === normalizedUrl &&
    Boolean(metadataPreview.data);

  const metadataErrorActive =
    metadataPreview.status === "error" &&
    metadataPreview.sourceUrl === normalizedUrl &&
    Boolean(metadataPreview.error);

  const linkStatus = useMemo(() => {
    if (!normalizedUrl) {
      return {
        color: "default",
        label: "等待输入",
        description: "输入链接后即可解析并预览信息。",
      };
    }

    if (!isUrlFormatValid) {
      return {
        color: "error",
        label: "链接格式不正确",
        description: "请输入以 http:// 或 https:// 开头的完整链接。",
      };
    }

    if (metadataLoading) {
      return {
        color: "processing",
        label: "正在解析",
        description: "正在尝试获取封面与标题，请稍候。",
      };
    }

    if (metadataResolved) {
      return {
        color: "success",
        label: "解析成功",
        description: "已获取到封面、标题与简介信息。",
      };
    }

    if (metadataErrorActive) {
      return {
        color: "warning",
        label: "解析失败",
        description: metadataPreview.error || "请检查链接或稍后重试。",
      };
    }

    return {
      color: "default",
      label: "待解析",
      description: "点击“解析”按钮或离开输入框触发解析。",
    };
  }, [
    normalizedUrl,
    isUrlFormatValid,
    metadataLoading,
    metadataResolved,
    metadataErrorActive,
    metadataPreview.error,
  ]);

  const parseMetadataForCurrentUrl = useCallback(async () => {
    if (!normalizedUrl || !isUrlFormatValid) {
      return;
    }

    if (
      metadataPreview.status === "loading" &&
      metadataPreview.sourceUrl === normalizedUrl
    ) {
      return;
    }

    if (
      metadataPreview.status === "success" &&
      metadataPreview.sourceUrl === normalizedUrl
    ) {
      return;
    }

    setMetadataPreview({
      status: "loading",
      data: null,
      error: "",
      sourceUrl: normalizedUrl,
    });

    try {
      const preview = await fetchMediaPreview(normalizedUrl);
      setMetadataPreview({
        status: "success",
        data: preview,
        error: "",
        sourceUrl: normalizedUrl,
      });
    } catch (err) {
      setMetadataPreview({
        status: "error",
        data: null,
        error: extractErrorMessage(err),
        sourceUrl: normalizedUrl,
      });
    }
  }, [
    normalizedUrl,
    isUrlFormatValid,
    metadataPreview.status,
    metadataPreview.sourceUrl,
  ]);

  const handleUrlBlur = () => {
    if (!normalizedUrl || !isUrlFormatValid) {
      return;
    }
    parseMetadataForCurrentUrl();
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

  const handleDownload = async (event) => {
    event.preventDefault();

    let trimmedUrl = "";
    try {
      const values = await form.validateFields(["url"]);
      trimmedUrl = (values.url || "").trim();
    } catch {
      return;
    }

    if (!trimmedUrl) {
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

  const showYtDlpWarningBadge = useMemo(() => {
    if (!settingsStatusSnapshot) {
      return false;
    }

    if (settingsStatusSnapshot.checkingYt) {
      return false;
    }

    return !settingsStatusSnapshot.ytInstalled;
  }, [settingsStatusSnapshot]);

  const renderPreviewContent = () => {
    if (!normalizedUrl) {
      return (
        <Empty
          description="粘贴链接即可在此处预览标题与封面"
          image={Empty.PRESENTED_IMAGE_SIMPLE}
        />
      );
    }

    if (!isUrlFormatValid) {
      return (
        <Empty
          description="请输入以 http(s) 开头的完整链接"
          image={<PictureOutlined style={{ fontSize: 36, color: "#94a3b8" }} />}
        />
      );
    }

    if (metadataLoading) {
      return <Skeleton active paragraph={{ rows: 3 }} title={{ width: "60%" }} />;
    }

    if (metadataErrorActive) {
      return (
        <Alert
          type="warning"
          showIcon
          message="解析失败"
          description={metadataPreview.error || "请检查链接或稍后重试。"}
        />
      );
    }

    if (metadataResolved && metadataPreview.data) {
      const { title, thumbnailUrl, description, provider, author } =
        metadataPreview.data;
      const sourceLabel = [provider, author].filter(Boolean).join(" · ");
      return (
        <Space direction="vertical" size="small" style={{ width: "100%" }}>
          <div className="media-preview__thumbnail">
            {thumbnailUrl ? (
              <img src={thumbnailUrl} alt={title} />
            ) : (
              <div className="media-preview__thumbnail--placeholder">
                <PictureOutlined style={{ fontSize: 28 }} />
                <Text type="secondary">暂无封面</Text>
              </div>
            )}
          </div>
          <Title level={5} style={{ marginBottom: 0 }}>
            {title}
          </Title>
          <Text type="secondary">{sourceLabel || "未知来源"}</Text>
          <Paragraph className="media-preview__description" ellipsis={{ rows: 3 }}>
            {description}
          </Paragraph>
        </Space>
      );
    }

    return (
      <Empty
        description="点击“解析”按钮获取预览信息"
        image={Empty.PRESENTED_IMAGE_SIMPLE}
      />
    );
  };

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
        <div className="app-shell">
          <Space direction="vertical" size="large" style={{ width: "100%" }}>
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

            <Card>
              <div className="download-layout">
                <div className="download-layout-main">
                  <Form
                    form={form}
                    layout="vertical"
                    initialValues={{ url: "" }}
                    onSubmitCapture={handleDownload}
                  >
                    <Form.Item
                      label="视频链接"
                      name="url"
                      validateTrigger={["onBlur", "onSubmit"]}
                      rules={[
                        { required: true, message: "请输入需要下载的视频链接" },
                        {
                          validator: (_, value) => {
                            if (!value || !value.trim()) {
                              return Promise.resolve();
                            }
                            return isLikelyUrl(value.trim())
                              ? Promise.resolve()
                              : Promise.reject(
                                  new Error("请输入合法的链接（需以 http/https 开头）")
                                );
                          },
                        },
                      ]}
                    >
                      <Input
                        ref={urlInputRef}
                        size="large"
                        placeholder="粘贴 YouTube 或其它站点的链接"
                        autoComplete="off"
                        className="url-input-with-action url-input-large"
                        onBlur={handleUrlBlur}
                        addonAfter={
                          <Button
                            type="default"
                            htmlType="button"
                            onClick={parseMetadataForCurrentUrl}
                            loading={metadataLoading}
                            disabled={!normalizedUrl || !isUrlFormatValid || metadataLoading}
                          >
                            解析
                          </Button>
                        }
                      />
                    </Form.Item>

                    <Form.Item label="下载类型">
                      <Radio.Group
                        value={downloadType}
                        onChange={(event) =>
                          setDownloadType(String(event.target.value))
                        }
                        className="download-type-radio"
                        disabled={isDownloading}
                      >
                        <Radio.Button value="video">
                          <Space size={6} align="center">
                            <LuSquarePlay size={18} strokeWidth={2} />
                            {`视频（${
                              VIDEO_QUALITY_LABELS[videoQuality] ?? "最高画质"
                            }）`}
                          </Space>
                        </Radio.Button>
                        <Radio.Button value="audio">
                          <Space size={6} align="center">
                            <LuHeadphones size={18} strokeWidth={2} />
                            纯音频 (MP3)
                          </Space>
                        </Radio.Button>
                      </Radio.Group>
                    </Form.Item>

                    <Form.Item
                      label={
                        <span className="form-label-with-help">
                          <span>Cookies 浏览器</span>
                          <Tooltip placement="top" title={COOKIE_TIP}>
                            <QuestionCircleOutlined className="form-label-help-icon" />
                          </Tooltip>
                        </span>
                      }
                    >
                      <Select
                        value={browser}
                        onChange={(value) => setBrowser(value)}
                        disabled={isDownloading}
                        options={COOKIE_SOURCE_OPTIONS}
                      />
                    </Form.Item>

                    <Form.Item label="保存位置">
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
                  </Form>
                </div>
                <div className="download-layout-side">
                  <div className="media-preview-panel">
                    <div className="media-preview-panel__header">
                      <div>
                        <Text strong>视频信息</Text>
                        {normalizedUrl && (
                          <Paragraph
                            className="media-preview-panel__url"
                            ellipsis={{ rows: 1 }}
                          >
                            {normalizedUrl}
                          </Paragraph>
                        )}
                      </div>
                      <Tag color={linkStatus.color === "default" ? undefined : linkStatus.color}>
                        {linkStatus.label}
                      </Tag>
                    </div>
                    <Text type="secondary" className="media-preview-panel__status-hint">
                      {linkStatus.description}
                    </Text>
                    <div className="media-preview-panel__body">
                      {renderPreviewContent()}
                    </div>
                  </div>
                </div>
              </div>
            </Card>
          </Space>
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

const fetchMediaPreview = async (targetUrl) => {
  const endpoint = `https://noembed.com/embed?nowrap=on&url=${encodeURIComponent(
    targetUrl
  )}`;
  const response = await fetch(endpoint);

  if (!response.ok) {
    throw new Error(`解析失败（${response.status}）`);
  }

  const payload = await response.json();
  if (payload?.error) {
    throw new Error(payload.error);
  }

  return {
    title: payload.title?.trim() || "未能获取标题",
    thumbnailUrl: payload.thumbnail_url || "",
    author: payload.author_name || "",
    provider: payload.provider_name || "",
    description:
      (payload.description && payload.description.trim()) ||
      payload.author_name ||
      payload.provider_name ||
      "暂无简介信息",
  };
};

const isLikelyUrl = (value) => {
  if (typeof value !== "string") {
    return false;
  }

  const trimmed = value.trim();
  if (!trimmed) {
    return false;
  }

  return /^https?:\/\/.+/i.test(trimmed);
};

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

export default App;
