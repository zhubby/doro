import type { ElementType } from "react";

export type NavigationItem = {
  label: string;
  href: string;
  description: string;
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

export type ResourceAction = {
  label: string;
  disabled?: boolean;
};

export type ResourceColumn<T> = {
  key: keyof T | string;
  label: string;
  className?: string;
  render?: (row: T) => React.ReactNode;
};

export type ContainerResource = {
  id: string;
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

export type SettingOption = {
  id: string;
  label: string;
  value: string;
  helper?: string;
  action?: string;
  choices?: string[];
};
