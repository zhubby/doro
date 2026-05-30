import {
  AppWindow,
  Boxes,
  CheckCircle2,
  Database,
  FolderTree,
  Home,
  ListTodo,
  MonitorCheck,
  Network,
  ScrollText,
  Server,
  Settings,
  ShieldCheck,
  Terminal,
  Zap,
} from "lucide-react";

import type { NavigationItem } from "@/types/dashboard";

export const navigation: NavigationItem[] = [
  {
    id: "overview",
    href: "/",
    icon: Home,
  },
  {
    id: "hosts",
    href: "/hosts",
    icon: Server,
  },
  {
    id: "tasks",
    href: "/tasks",
    icon: ListTodo,
  },
  {
    id: "approvals",
    href: "/approvals",
    icon: ShieldCheck,
  },
  {
    id: "apps",
    href: "/apps",
    icon: AppWindow,
    count: 4,
  },
  {
    id: "ai",
    href: "/ai",
    icon: Zap,
  },
  {
    id: "terminal",
    href: "/terminal",
    icon: Terminal,
  },
  {
    id: "files",
    href: "/files",
    icon: FolderTree,
  },
  {
    id: "websites",
    href: "/websites",
    icon: Network,
    count: 1,
  },
  {
    id: "databases",
    href: "/databases",
    icon: Database,
    count: 2,
  },
  {
    id: "containers",
    href: "/containers",
    icon: Boxes,
  },
  {
    id: "system",
    href: "/system",
    icon: MonitorCheck,
  },
  {
    id: "logs",
    href: "/logs",
    icon: ScrollText,
  },
  {
    id: "cron",
    href: "/cron",
    icon: CheckCircle2,
  },
  {
    id: "settings",
    href: "/settings",
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
