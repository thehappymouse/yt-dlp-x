import { forwardRef, useCallback, useEffect, useImperativeHandle, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { DownloadOutlined, RedoOutlined } from "@ant-design/icons";
import { Alert, Button, Flex, Modal, Radio, Space, Tag, Tooltip, Typography } from "antd";
import { LuCircleAlert, LuCircleCheck, LuLoaderCircle } from "react-icons/lu";
import {
  SiYoutube,
  SiBilibili,
  SiTiktok,
  SiInstagram,
  SiVimeo,
  SiX,
  SiFacebook,
  SiSoundcloud,
} from "react-icons/si";
import { extractErrorMessage } from "./utils/errors";

const { Text } = Typography;

const VIDEO_QUALITY_OPTIONS = [
  { label: "低画质", value: "low" },
  { label: "中画质", value: "medium" },
  { label: "最高画质", value: "highest" },
];

const VIDEO_QUALITY_HINT =
  "低画质：优先尝试 480P 及以下；中画质：优先获取 1080P；最高画质：尝试最高可用画质（Bilibili 大会员可解锁 4K）。";

const SUPPORTED_SITES = [
  { label: "YouTube", Icon: SiYoutube, color: "#ff0000" },
  { label: "Bilibili", Icon: SiBilibili, color: "#00A1D6" },
  { label: "TikTok", Icon: SiTiktok, color: "#000000" },
  { label: "Instagram", Icon: SiInstagram, color: "#E4405F" },
  { label: "Vimeo", Icon: SiVimeo, color: "#1AB7EA" },
  { label: "Twitter", Icon: SiX, color: "#1DA1F2" },
  { label: "Facebook", Icon: SiFacebook, color: "#1877F2" },
  { label: "SoundCloud", Icon: SiSoundcloud, color: "#FF5500" },
];

const SettingsModal = forwardRef(function SettingsModal(
  {
    open,
    onClose,
    isDownloading,
    onStatusChange,
    videoQuality = "highest",
    onVideoQualityChange,
  },
  ref
) {
  const [ytStatus, setYtStatus] = useState({
    installed: false,
    path: "",
    source: "",
  });
  const [ffStatus, setFfStatus] = useState({
    installed: false,
    path: "",
    source: "",
  });
  const [checkingYt, setCheckingYt] = useState(true);
  const [checkingFf, setCheckingFf] = useState(true);
  const [isInstallingYt, setIsInstallingYt] = useState(false);
  const [isInstallingFf, setIsInstallingFf] = useState(false);
  const [feedback, setFeedback] = useState(null);

  const refreshYtStatus = useCallback(async () => {
    try {
      setCheckingYt(true);
      const status = await invoke("check_yt_dlp");
      setYtStatus({
        installed: Boolean(status.installed),
        path: status.path ?? "",
        source: status.source ?? "",
      });
    } catch (err) {
      setFeedback({
        type: "error",
        message: `检测 yt-dlp 失败：${extractErrorMessage(err)}`,
      });
    } finally {
      setCheckingYt(false);
    }
  }, []);

  const refreshFfmpegStatus = useCallback(async () => {
    try {
      setCheckingFf(true);
      const status = await invoke("check_ffmpeg");
      setFfStatus({
        installed: Boolean(status.installed),
        path: status.path ?? "",
        source: status.source ?? "",
      });
    } catch (err) {
      setFeedback({
        type: "error",
        message: `检测 ffmpeg 失败：${extractErrorMessage(err)}`,
      });
    } finally {
      setCheckingFf(false);
    }
  }, []);

  const refreshBinaryStatuses = useCallback(() => {
    refreshYtStatus();
    refreshFfmpegStatus();
  }, [refreshYtStatus, refreshFfmpegStatus]);

  const handleVideoQualityChange = useCallback(
    (value) => {
      if (typeof value !== "string") {
        return;
      }

      if (onVideoQualityChange) {
        onVideoQualityChange(value);
      }
    },
    [onVideoQualityChange]
  );

  useImperativeHandle(
    ref,
    () => ({
      refreshStatuses: refreshBinaryStatuses,
    }),
    [refreshBinaryStatuses]
  );

  useEffect(() => {
    refreshBinaryStatuses();
  }, [refreshBinaryStatuses]);

  useEffect(() => {
    if (open) {
      refreshBinaryStatuses();
    }
  }, [open, refreshBinaryStatuses]);

  const installYtDlp = useCallback(async () => {
    setIsInstallingYt(true);
    setFeedback(null);
    try {
      const status = await invoke("install_yt_dlp");
      setYtStatus({
        installed: Boolean(status.installed),
        path: status.path ?? "",
        source: status.source ?? "",
      });
      setFeedback({
        type: "success",
        message: "yt-dlp 已下载并可用。",
      });
    } catch (err) {
      setFeedback({
        type: "error",
        message: `安装 yt-dlp 失败：${extractErrorMessage(err)}`,
      });
    } finally {
      setIsInstallingYt(false);
      refreshYtStatus();
    }
  }, [refreshYtStatus]);

  const installFfmpeg = useCallback(async () => {
    setIsInstallingFf(true);
    setFeedback(null);
    try {
      const status = await invoke("install_ffmpeg");
      setFfStatus({
        installed: Boolean(status.installed),
        path: status.path ?? "",
        source: status.source ?? "",
      });
      setFeedback({
        type: "success",
        message: "ffmpeg 已下载并可用。",
      });
    } catch (err) {
      setFeedback({
        type: "error",
        message: `安装 ffmpeg 失败：${extractErrorMessage(err)}`,
      });
    } finally {
      setIsInstallingFf(false);
      refreshFfmpegStatus();
    }
  }, [refreshFfmpegStatus]);

  const statusSnapshot = useMemo(
    () => ({
      ytInstalled: ytStatus.installed,
      ffInstalled: ffStatus.installed,
      checkingYt,
      checkingFf,
    }),
    [ytStatus.installed, ffStatus.installed, checkingYt, checkingFf]
  );

  useEffect(() => {
    if (onStatusChange) {
      onStatusChange(statusSnapshot);
    }
  }, [onStatusChange, statusSnapshot]);

  const ytStatusLabel = checkingYt
    ? "正在检测 yt-dlp..."
    : ytStatus.installed
    ? `yt-dlp 已就绪（${
        ytStatus.source === "system" ? "系统版本" : "内置版本"
      }）`
    : "尚未检测到 yt-dlp";

  const ytStatusTagColor = checkingYt
    ? "processing"
    : ytStatus.installed
    ? "success"
    : "warning";

  const ytStatusTagIcon = checkingYt ? (
    <LuLoaderCircle className="status-icon icon-spin" size={14} strokeWidth={2.75} />
  ) : ytStatus.installed ? (
    <LuCircleCheck className="status-icon" size={14} strokeWidth={2.5} />
  ) : (
    <LuCircleAlert className="status-icon" size={14} strokeWidth={2.5} />
  );

  const ffSourceLabel =
    ffStatus.source === "system"
      ? "系统版本"
      : ffStatus.source === "bundled"
      ? "内置版本"
      : "";

  const ffStatusLabel = checkingFf
    ? "正在检测 ffmpeg..."
    : ffStatus.installed
    ? `ffmpeg 已就绪${ffSourceLabel ? `（${ffSourceLabel}）` : ""}`
    : "尚未检测到 ffmpeg";

  const ffStatusTagColor = checkingFf
    ? "processing"
    : ffStatus.installed
    ? "success"
    : "warning";

  const ffStatusTagIcon = checkingFf ? (
    <LuLoaderCircle className="status-icon icon-spin" size={14} strokeWidth={2.75} />
  ) : ffStatus.installed ? (
    <LuCircleCheck className="status-icon" size={14} strokeWidth={2.5} />
  ) : (
    <LuCircleAlert className="status-icon" size={14} strokeWidth={2.5} />
  );

  const ytStatusHelperText = checkingYt
    ? "正在检测系统中的 yt-dlp..."
    : ytStatus.path
    ? `当前使用的 yt-dlp 路径：${ytStatus.path}`
    : "将自动在首次下载时获取 yt-dlp。";

  const ffStatusHelperText = checkingFf
    ? "正在检测系统中的 ffmpeg..."
    : ffStatus.path
    ? `当前使用的 ffmpeg 路径：${ffStatus.path}`
    : "未检测到 ffmpeg，请先通过下方按钮安装或在系统中安装，以支持音频转换与封面嵌入。";

  const refreshDisabled =
    checkingYt ||
    checkingFf ||
    isInstallingYt ||
    isInstallingFf ||
    isDownloading;

  const closeModal = useCallback(() => {
    if (onClose) {
      onClose();
    }
  }, [onClose]);

  return (
    <Modal
      title="设置"
      open={open}
      onCancel={closeModal}
      footer={null}
      centered
      width="70%"
      destroyOnHidden={false}
    >
      <Space direction="vertical" size="large" style={{ width: "100%" }}>
        <Space direction="vertical" size="small" style={{ width: "100%" }}>
          <Flex align="center" justify="space-between" wrap="wrap" gap="small">
            <Space align="center" size="small" wrap>
              <Tag color={ytStatusTagColor} icon={ytStatusTagIcon} bordered>
                {ytStatusLabel}
              </Tag>
              <Tag color={ffStatusTagColor} icon={ffStatusTagIcon} bordered>
                {ffStatusLabel}
              </Tag>
            </Space>
            <Space wrap>
              <Button
                icon={<RedoOutlined />}
                onClick={refreshBinaryStatuses}
                disabled={refreshDisabled}
                loading={checkingYt || checkingFf}
              >
                重新检测
              </Button>
              <Button
                type="primary"
                ghost
                icon={<DownloadOutlined />}
                onClick={installYtDlp}
                loading={isInstallingYt}
                disabled={isInstallingFf || isDownloading}
              >
                安装 / 更新 yt-dlp
              </Button>
              <Button
                type="primary"
                ghost
                icon={<DownloadOutlined />}
                onClick={installFfmpeg}
                loading={isInstallingFf}
                disabled={isInstallingYt || isDownloading}
              >
                安装 / 更新 ffmpeg
              </Button>
            </Space>
          </Flex>
          <Text type="secondary">{ytStatusHelperText}</Text>
          <Text type="secondary">{ffStatusHelperText}</Text>
        </Space>
        <Space direction="vertical" size="small" style={{ width: "100%" }}>
          <Text strong>默认视频画质</Text>
          <Radio.Group
            className="radio-card-group video-quality-radio-group"
            value={videoQuality}
            onChange={(event) => handleVideoQualityChange(event.target.value)}
            disabled={isDownloading}
          >
            {VIDEO_QUALITY_OPTIONS.map(({ label, value }) => (
              <Radio key={value} value={value}>
                <span className="radio-option-label">{label}</span>
              </Radio>
            ))}
          </Radio.Group>
          <Text type="secondary">{VIDEO_QUALITY_HINT}</Text>
        </Space>
        <Space direction="vertical" size="small" style={{ width: "100%" }}>
          <Text strong>支持的网站</Text>
          <Text type="secondary">当前版本已针对以下站点进行适配：</Text>
          <div className="supported-sites-list">
            {SUPPORTED_SITES.map(({ label, Icon, color }) => (
              <Tooltip key={label} title={label}>
                <span className="supported-site-icon" aria-label={label}>
                  <Icon size={22} color={color} />
                </span>
              </Tooltip>
            ))}
          </div>
        </Space>
        {feedback?.message ? (
          <Alert type={feedback.type} showIcon message={feedback.message} />
        ) : null}
      </Space>
    </Modal>
  );
});

export default SettingsModal;
