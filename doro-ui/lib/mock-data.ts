import { Activity, Database, Layers3, Network } from "lucide-react";

import type {
  Application,
  ContainerResource,
  DatabaseResource,
  Metric,
  OverviewStat,
  SettingOption,
  SystemStat,
} from "@/types/dashboard";

export const overviewStats: OverviewStat[] = [
  { label: "智能体", value: "0", helper: "待接入" },
  { label: "网站", value: "1", helper: "1 个运行中" },
  { label: "数据库", value: "2", helper: "MySQL / Redis" },
  { label: "已安装应用", value: "6", helper: "2 个可更新" },
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
  "本周检查应用更新与备份策略。",
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
