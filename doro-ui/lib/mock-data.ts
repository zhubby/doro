import { Activity, Database, Layers3, Network } from "lucide-react";

import type {
  Application,
  ContainerResource,
  CronJob,
  DatabaseResource,
  AiAgent,
  Metric,
  OverviewStat,
  SettingOption,
  SystemInfo,
  SystemMetric,
  SystemStat,
  TerminalSession,
  ToolEntry,
  VirtualMachineResource,
  WebsiteResource,
} from "@/types/dashboard";

export const overviewStats: OverviewStat[] = [
  { label: "智能体", value: "0", helper: "待接入" },
  { label: "网站", value: "1", helper: "1 个运行中" },
  { label: "数据库", value: "2", helper: "MySQL / Redis" },
  { label: "容器", value: "4", helper: "3 个运行中" },
];

export const systemStats: SystemStat[] = [
  { label: "负载", value: "4.83%", detail: "运行流畅", progress: 5 },
  { label: "CPU", value: "4.23%", detail: "0.17 / 4 核", progress: 4 },
  { label: "内存", value: "17.24%", detail: "1.29 GB / 7.31 GB", progress: 17 },
  { label: "磁盘", value: "17.91%", detail: "33.2 GB / 186.49 GB", progress: 18 },
];

export const trafficMetrics: Metric[] = [
  { label: "上行", value: "4.02 KB/s" },
  { label: "下行", value: "8.66 KB/s" },
  { label: "总发送", value: "1.06 TB" },
  { label: "总接收", value: "139.65 GB" },
];

export const diskMetrics: Metric[] = [
  { label: "读取", value: "18.4 MB/s" },
  { label: "写入", value: "6.8 MB/s" },
  { label: "IO 使用率", value: "12%" },
  { label: "等待", value: "2 ms" },
];

export const trendBars = [18, 26, 22, 34, 28, 44, 38, 52, 46, 58, 48, 64];

export const notes = [
  "本周检查虚拟机快照与备份策略。",
  "开启安全入口后再暴露公网访问。",
  "数据库监控阈值保持默认配置。",
];

export const applications: Application[] = [
  {
    id: "openresty",
    name: "OpenResty",
    version: "1.27.1.2-0.1-local",
    description: "高性能 Web 服务运行环境",
    category: "网站",
    state: "running",
    icon: Network,
  },
  {
    id: "mysql",
    name: "MySQL",
    version: "8.0.37",
    description: "关系型数据库服务",
    category: "数据库",
    state: "upgrade",
    icon: Database,
    updateAvailable: true,
  },
  {
    id: "redis",
    name: "Redis",
    version: "7.2.5",
    description: "内存键值缓存服务",
    category: "数据库",
    state: "available",
    icon: Layers3,
  },
  {
    id: "hermes",
    name: "Hermes Agent",
    version: "0.4.1",
    description: "智能体调度与任务执行",
    category: "AI",
    state: "available",
    icon: Activity,
  },
];

export const containers: ContainerResource[] = [
  {
    id: "openresty",
    name: "doro-openresty",
    image: "openresty:1.27.1",
    status: "running",
    source: "应用商店",
    cpu: "1.42%",
    memory: "86 MB",
    ports: "80:80 / 443:443",
    updatedAt: "刚刚",
  },
  {
    id: "mysql",
    name: "doro-mysql",
    image: "mysql:8.0",
    status: "running",
    source: "应用商店",
    cpu: "2.18%",
    memory: "412 MB",
    ports: "3306:3306",
    updatedAt: "5 分钟前",
  },
  {
    id: "redis",
    name: "doro-redis",
    image: "redis:7.2",
    status: "stopped",
    source: "手动创建",
    cpu: "0.00%",
    memory: "0 MB",
    ports: "6379:6379",
    updatedAt: "昨天",
  },
  {
    id: "n8n",
    name: "doro-n8n",
    image: "n8nio/n8n:1.92",
    status: "warning",
    source: "手动创建",
    cpu: "4.78%",
    memory: "768 MB",
    ports: "5678:5678",
    updatedAt: "18 分钟前",
  },
];

export const virtualMachines: VirtualMachineResource[] = [
  {
    id: "vm-home-assistant",
    name: "home-assistant",
    status: "running",
    host: "doro-node-01",
    image: "Debian 12 / HAOS",
    cpu: "2 vCPU · 18%",
    memory: "4 GB · 62%",
    disk: "64 GB · 28%",
    address: "10.0.1.24",
    uptime: "12 天 4 小时",
    updatedAt: "刚刚",
  },
  {
    id: "vm-devbox",
    name: "devbox",
    status: "running",
    host: "doro-node-01",
    image: "Ubuntu 24.04 LTS",
    cpu: "4 vCPU · 36%",
    memory: "8 GB · 51%",
    disk: "160 GB · 47%",
    address: "10.0.1.31",
    uptime: "3 天 7 小时",
    updatedAt: "5 分钟前",
  },
  {
    id: "vm-media",
    name: "media-stack",
    status: "warning",
    host: "doro-node-02",
    image: "Fedora Server 40",
    cpu: "6 vCPU · 72%",
    memory: "12 GB · 81%",
    disk: "1.2 TB · 86%",
    address: "10.0.1.42",
    uptime: "24 天 1 小时",
    updatedAt: "16 分钟前",
  },
  {
    id: "vm-lab",
    name: "security-lab",
    status: "stopped",
    host: "doro-node-02",
    image: "Kali 2025.1",
    cpu: "2 vCPU · 0%",
    memory: "4 GB · 0%",
    disk: "96 GB · 34%",
    address: "未分配",
    uptime: "已停止",
    updatedAt: "昨天",
  },
];

export const databases: DatabaseResource[] = [
  {
    id: "mysql-main",
    name: "mysql-main",
    engine: "MySQL",
    status: "running",
    version: "8.0.37",
    size: "1.8 GB",
    backup: "今天 03:00",
    updatedAt: "2 分钟前",
  },
  {
    id: "redis-cache",
    name: "redis-cache",
    engine: "Redis",
    status: "warning",
    version: "7.2.5",
    size: "318 MB",
    backup: "未开启",
    updatedAt: "1 小时前",
  },
];

export const panelSettings: SettingOption[] = [
  {
    id: "theme",
    label: "主题",
    value: "跟随系统",
    helper: "切换浅色、深色或系统自动模式。",
    action: "设置",
    choices: ["浅色", "深色", "跟随系统"],
  },
  {
    id: "entrance",
    label: "安全入口",
    value: "已启用",
    helper: "隐藏默认登录路径，降低暴露风险。",
    action: "查看",
  },
  {
    id: "panel-name",
    label: "面板名称",
    value: "Doro Panel",
    helper: "显示在浏览器标题和侧边栏顶部。",
    action: "编辑",
  },
  {
    id: "language",
    label: "语言",
    value: "简体中文",
    helper: "当前界面语言。",
    action: "切换",
    choices: ["简体中文", "English"],
  },
  {
    id: "session",
    label: "会话超时",
    value: "86400 秒",
    helper: "超过该时间未操作后需要重新登录。",
    action: "设置",
  },
  {
    id: "api",
    label: "API 接口",
    value: "已关闭",
    helper: "开启后允许外部系统通过令牌访问面板 API。",
    action: "启用",
  },
];

export const websites: WebsiteResource[] = [
  {
    id: "doro-home",
    primaryDomain: "doro.local",
    status: "running",
    runtime: "OpenResty",
    ssl: "自签证书 · 89 天后过期",
    rootPath: "/opt/doro/www",
    traffic: "12.4 MB / 今日",
    updatedAt: "刚刚",
  },
  {
    id: "agent-preview",
    primaryDomain: "agent.doro.local",
    status: "warning",
    runtime: "反向代理",
    ssl: "未配置",
    rootPath: "http://127.0.0.1:8787",
    traffic: "1.8 MB / 今日",
    updatedAt: "12 分钟前",
  },
];

export const systemMetrics: SystemMetric[] = [
  { label: "CPU", value: "4.23%", detail: "4 核 / 0.17 负载", progress: 4 },
  { label: "内存", value: "17.24%", detail: "1.29 GB / 7.31 GB", progress: 17 },
  { label: "磁盘", value: "17.91%", detail: "33.2 GB / 186.49 GB", progress: 18 },
  { label: "网络", value: "8.66 KB/s", detail: "下行实时速率", progress: 9 },
];

export const systemInfo: SystemInfo[] = [
  { label: "主机名", value: "doro-local", helper: "本地开发节点" },
  { label: "系统", value: "macOS 25.5.0", helper: "darwin arm64" },
  { label: "运行时间", value: "12 天 4 小时", helper: "自上次重启后" },
  { label: "安全入口", value: "已启用", helper: "隐藏默认登录路径" },
];

export const cronJobs: CronJob[] = [
  {
    id: "backup-db",
    name: "数据库备份",
    type: "备份",
    schedule: "每天 03:00",
    status: "running",
    lastRun: "今天 03:00",
    retention: "保留 7 份",
  },
  {
    id: "security-check",
    name: "安全巡检",
    type: "巡检",
    schedule: "每 6 小时",
    status: "running",
    lastRun: "2 小时前",
    retention: "保留 30 天日志",
  },
  {
    id: "clean-cache",
    name: "清理临时文件",
    type: "清理",
    schedule: "每周日 02:00",
    status: "stopped",
    lastRun: "上周日",
    retention: "无产物",
  },
];

export const tools: ToolEntry[] = [
  {
    id: "process",
    title: "进程管理",
    description: "查看系统进程、资源占用和异常任务。",
    category: "系统",
    status: "ready",
  },
  {
    id: "network",
    title: "网络诊断",
    description: "执行端口、DNS、连通性和路由检查。",
    category: "诊断",
    status: "ready",
  },
  {
    id: "logs",
    title: "日志查看",
    description: "集中查看面板、应用和任务日志。",
    category: "日志",
    status: "ready",
  },
  {
    id: "files",
    title: "文件工具",
    description: "管理常用目录、上传下载与权限检查。",
    category: "文件",
    status: "beta",
  },
  {
    id: "firewall",
    title: "防火墙",
    description: "统一查看入站规则和开放端口。",
    category: "安全",
    status: "locked",
  },
];

export const terminalSessions: TerminalSession[] = [
  {
    id: "local",
    name: "本地终端",
    target: "127.0.0.1",
    status: "running",
    user: "zhubby",
    lastActive: "刚刚",
  },
  {
    id: "builder",
    name: "构建节点",
    target: "10.0.0.12",
    status: "warning",
    user: "deploy",
    lastActive: "18 分钟前",
  },
];

export const aiAgents: AiAgent[] = [
  {
    id: "ops-copilot",
    name: "运维助手",
    role: "巡检与故障摘要",
    status: "running",
    model: "DeepSeek Reasoner",
    lastRun: "5 分钟前",
  },
  {
    id: "release-writer",
    name: "发布说明生成器",
    role: "根据提交生成变更说明",
    status: "stopped",
    model: "GPT",
    lastRun: "昨天",
  },
  {
    id: "log-analyst",
    name: "日志分析员",
    role: "归纳异常日志和重试建议",
    status: "warning",
    model: "Local LLM",
    lastRun: "1 小时前",
  },
];
