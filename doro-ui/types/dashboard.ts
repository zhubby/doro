import type { ElementType } from "react";

export type NavigationItem = {
  id:
    | "overview"
    | "hosts"
    | "tasks"
    | "approvals"
    | "apps"
    | "ai"
    | "terminal"
    | "files"
    | "websites"
    | "databases"
    | "containers"
    | "system"
    | "logs"
    | "cron"
    | "settings";
  href: string;
  icon: ElementType;
  count?: number;
};

export type AppState = "installed" | "available" | "running" | "upgrade";

export type ThemeMode = "light" | "dark";

export type OverviewStat = {
  label: string;
  value: string;
  helper: string;
};

export type SystemStat = {
  label: string;
  value: string;
  detail: string;
  progress: number;
};

export type Metric = {
  label: string;
  value: string;
};

export type Application = {
  id: string;
  name: string;
  version: string;
  description: string;
  category: string;
  state: AppState;
  icon: ElementType;
  updateAvailable?: boolean;
};

export type ResourceStatus = "running" | "stopped" | "warning";

export type VirtualMachineResource = {
  id: string;
  name: string;
  status: ResourceStatus;
  host: string;
  image: string;
  cpu: string;
  memory: string;
  disk: string;
  address: string;
  uptime: string;
  updatedAt: string;
};

export type ResourceAction = {
  label: string;
  disabled?: boolean;
};

export type ResourceColumn<T> = {
  key: keyof T | string;
  label: string;
  className?: string;
  width?: string;
  render?: (row: T) => React.ReactNode;
};

export type ContainerResource = {
  id: string;
  hostId: string;
  agentName: string;
  name: string;
  image: string;
  status: ResourceStatus;
  source: string;
  cpu: string;
  memory: string;
  ports: string;
  updatedAt: string;
};

export type DatabaseResource = {
  id: string;
  name: string;
  engine: string;
  status: ResourceStatus;
  version: string;
  size: string;
  backup: string;
  updatedAt: string;
};

export type WebsiteResource = {
  id: string;
  primaryDomain: string;
  status: ResourceStatus;
  runtime: string;
  ssl: string;
  rootPath: string;
  traffic: string;
  updatedAt: string;
};

export type SystemMetric = {
  label: string;
  value: string;
  detail: string;
  progress: number;
};

export type SystemInfo = {
  label: string;
  value: string;
  helper: string;
};

export type CronJob = {
  id: string;
  name: string;
  type: string;
  schedule: string;
  status: ResourceStatus;
  lastRun: string;
  retention: string;
};

export type ToolEntry = {
  id: string;
  title: string;
  description: string;
  category: string;
  status: "ready" | "beta" | "locked";
};

export type TerminalSession = {
  id: string;
  name: string;
  target: string;
  status: ResourceStatus;
  user: string;
  lastActive: string;
};

export type AiAgent = {
  id: string;
  name: string;
  role: string;
  status: ResourceStatus;
  model: string;
  lastRun: string;
};

export type SettingOption = {
  id: string;
  label: string;
  value: string;
  helper?: string;
  action?: string;
  choices?: string[];
};
