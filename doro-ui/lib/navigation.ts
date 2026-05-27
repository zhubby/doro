import {
  AppWindow,
  Boxes,
  CheckCircle2,
  Database,
  Home,
  ListTodo,
  MonitorCheck,
  Network,
  ScrollText,
  Server,
  Settings,
  ShieldCheck,
  Zap,
} from "lucide-react";

import type { NavigationItem } from "@/types/dashboard";

export const navigation: NavigationItem[] = [
  {
    label: "概览",
    href: "/",
    description: "统一查看站点、数据库、应用和服务器运行状态。",
    icon: Home,
  },
  {
    label: "主机",
    href: "/hosts",
    description: "查看已注册 Agent、主机状态和能力声明。",
    icon: Server,
  },
  {
    label: "任务",
    href: "/tasks",
    description: "跟踪任务计划、执行步骤和运行结果。",
    icon: ListTodo,
  },
  {
    label: "审批",
    href: "/approvals",
    description: "处理高风险主机操作和 AI 计划审批。",
    icon: ShieldCheck,
  },
  {
    label: "应用",
    href: "/apps",
    description: "浏览、安装和管理常用服务应用。",
    icon: AppWindow,
    count: 6,
  },
  {
    label: "AI",
    href: "/ai",
    description: "管理智能体运行环境和任务能力。",
    icon: Zap,
  },
  {
    label: "网站",
    href: "/websites",
    description: "查看网站、域名、证书和服务状态。",
    icon: Network,
    count: 1,
  },
  {
    label: "数据库",
    href: "/databases",
    description: "管理数据库实例、备份和连接状态。",
    icon: Database,
    count: 2,
  },
  {
    label: "容器",
    href: "/containers",
    description: "管理容器生命周期、镜像和资源使用。",
    icon: Boxes,
  },
  {
    label: "系统",
    href: "/system",
    description: "查看主机资源、磁盘、网络和安全状态。",
    icon: MonitorCheck,
  },
  {
    label: "日志",
    href: "/logs",
    description: "集中查看控制面、Agent 和任务日志。",
    icon: ScrollText,
  },
  {
    label: "计划任务",
    href: "/cron",
    description: "查看备份、巡检和自动化任务。",
    icon: CheckCircle2,
  },
  {
    label: "面板设置",
    href: "/settings",
    description: "配置主题、安全入口、会话和 API 访问。",
    icon: Settings,
  },
];

export function getNavigationItem(pathname: string) {
  return (
    navigation
      .filter((item) =>
        item.href === "/"
          ? pathname === "/"
          : pathname === item.href || pathname.startsWith(`${item.href}/`),
      )
      .sort((a, b) => b.href.length - a.href.length)[0] ?? navigation[0]
  );
}
