import { Activity, Database, Layers3, Network } from "lucide-react";

import type { AppSummary } from "@/types/api";
import type { Application, AppState } from "@/types/dashboard";

function appState(status: string): AppState {
  if (status === "running") {
    return "running";
  }

  if (status === "installed") {
    return "installed";
  }

  if (status === "upgrade") {
    return "upgrade";
  }

  return "available";
}

function appIcon(category: string) {
  if (category === "database") {
    return Database;
  }

  if (category === "website") {
    return Network;
  }

  if (category === "ai") {
    return Activity;
  }

  return Layers3;
}

function categoryLabel(category: string) {
  if (category === "database") {
    return "数据库";
  }

  if (category === "website") {
    return "网站";
  }

  if (category === "ai") {
    return "AI";
  }

  return category;
}

export function toApplications(apps: AppSummary[]): Application[] {
  return apps.map((app) => ({
    id: app.id,
    name: app.name,
    version: app.status,
    description: `${categoryLabel(app.category)} · ${app.status}`,
    category: categoryLabel(app.category),
    state: appState(app.status),
    icon: appIcon(app.category),
    updateAvailable: app.status === "upgrade",
  }));
}

