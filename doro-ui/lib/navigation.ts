import {
  AppWindow,
  BellRing,
  Bot,
  Boxes,
  CheckCircle2,
  Container,
  FolderTree,
  Home,
  Images,
  Layers3,
  MonitorCheck,
  MonitorPlay,
  Network,
  ScrollText,
  Server,
  Mail,
  ShipWheel,
  ShieldCheck,
  SquareStack,
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
    id: "virtualMachines",
    href: "/virtual-machines/instances",
    icon: AppWindow,
    children: [
      {
        id: "virtualMachineInstances",
        href: "/virtual-machines/instances",
        icon: MonitorPlay,
      },
      {
        id: "virtualMachineImages",
        href: "/virtual-machines/images",
        icon: Images,
      },
      {
        id: "virtualMachineSnapshots",
        href: "/virtual-machines/snapshots",
        icon: SquareStack,
      },
      {
        id: "virtualMachineTemplates",
        href: "/virtual-machines/templates",
        icon: Layers3,
      },
    ],
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
    id: "alerts",
    href: "/alerts",
    icon: BellRing,
  },
  {
    id: "notifications",
    href: "/notifications",
    icon: Mail,
  },
];

export function getNavigationItem(pathname: string) {
  if (pathname === "/apps") {
    return navigation
      .flatMap((item) => [item, ...(item.children ?? [])])
      .find((item) => item.id === "virtualMachines") ?? navigation[0];
  }
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
