import {
  AppWindow,
  Bot,
  Boxes,
  CheckCircle2,
  Container,
  FolderTree,
  Home,
  Layers3,
  MonitorCheck,
  Network,
  ScrollText,
  Server,
  Settings,
  ShipWheel,
  ShieldCheck,
  Terminal,
  Waypoints,
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
    icon: CheckCircle2,
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
    icon: Bot,
  },
  {
    id: "models",
    href: "/models",
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
    id: "docker",
    href: "/docker/containers",
    icon: Boxes,
    children: [
      {
        id: "dockerContainers",
        href: "/docker/containers",
        icon: Container,
      },
      {
        id: "dockerImages",
        href: "/docker/images",
        icon: Layers3,
      },
      {
        id: "dockerNetworks",
        href: "/docker/networks",
        icon: Waypoints,
      },
      {
        id: "dockerVolumes",
        href: "/docker/volumes",
        icon: Boxes,
      },
      {
        id: "dockerCompose",
        href: "/docker/compose",
        icon: ShipWheel,
      },
    ],
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
    id: "settings",
    href: "/settings",
    icon: Settings,
  },
];

export function getNavigationItem(pathname: string) {
  return (
    navigation
      .flatMap((item) => [item, ...(item.children ?? [])])
      .filter((item) =>
        item.href === "/"
          ? pathname === "/"
          : pathname === item.href || pathname.startsWith(`${item.href}/`),
      )
      .sort((a, b) => b.href.length - a.href.length)[0] ?? navigation[0]
  );
}
