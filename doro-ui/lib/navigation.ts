import {
  AppWindow,
  Boxes,
  CheckCircle2,
  Database,
  Home,
  Network,
  Server,
  Settings,
  SquareTerminal,
  Wrench,
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
    label: "应用商店",
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
    icon: Server,
  },
  {
    label: "终端",
    href: "/terminal",
    description: "连接本地或远程主机终端。",
    icon: SquareTerminal,
  },
  {
    label: "计划任务",
    href: "/cron",
    description: "查看备份、巡检和自动化任务。",
    icon: CheckCircle2,
  },
  {
    label: "工具箱",
    href: "/tools",
    description: "打开系统工具、诊断工具和快捷入口。",
    icon: Wrench,
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
