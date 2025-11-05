import { Button, List, Modal, Space, Tag, Typography } from "antd";
import { GithubOutlined, MailOutlined } from "@ant-design/icons";
import logo from "./assets/logo.png";
import { APP_VERSION } from "./version";

const { Title, Text, Paragraph } = Typography;

const AUTHORS = [
  { name: "@thehappymouse", role: "创意与打磨" },
  { name: "ChatGPT", role: "CTO" },
];

const LogoMark = (props) => <img src={logo} alt="yt-dlp-x logo" {...props} />;

const About = ({ open, onClose }) => (
  <Modal
    open={open}
    onCancel={onClose}
    footer={null}
    centered
    title={
      <Space align="center">
        <LogoMark className="about-logo" />

        <Title level={4} style={{ margin: 0 }}>
          关于 yt-dlp-x
        </Title>
      </Space>
    }
  >
    <Space direction="vertical" size="middle" style={{ width: "100%" }}>
      <Paragraph>
        yt-dlp-x 基于 Tauri 2 构建，提供直观的界面，让 yt-dlp 的强大能力更易于使用，支持音视频分离下载、Cookies 整合等特性。
      </Paragraph>
      <Tag color="geekblue" bordered style={{ alignSelf: "flex-start" }}>
        {APP_VERSION}
      </Tag>
      <Space direction="vertical" size={4}>

          type="link"
          href="https://github.com/thehappymouse/yt-dlp-x"
          target="_blank"
          rel="noreferrer"
          icon={<GithubOutlined />}
        >
          GitHub (https://github.com/thehappymouse/yt-dlp-x)
        </Button>
        <Button
          type="link"
          href="mailto:thehappymouse@gmail.com"
          icon={<MailOutlined />}
        >
          thehappymouse@gmail.com
        </Button>
      </Space>
      <div>
        <Title level={5} style={{ marginBottom: 12 }}>
          制作团队
        </Title>
        <List
          dataSource={AUTHORS}
          renderItem={(author) => (
            <List.Item key={author.name} className="author-item">
              <Space direction="vertical" size={0}>
                <Text strong>{author.name}</Text>
                {author.role ? <Text type="secondary">{author.role}</Text> : null}
              </Space>
            </List.Item>
          )}
          split={false}
        />
      </div>
    </Space>
  </Modal>
);

export default About;
