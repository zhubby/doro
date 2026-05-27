"use client";

import { useEffect, useState } from "react";

import { toApplications } from "@/lib/control-plane-mappers";
import {
  getApps,
  getApprovals,
  getHosts,
  getSettings,
  getTasks,
} from "@/lib/control-plane-api";
import { OverviewPage } from "@/components/dashboard/overview/overview-page";
import { HostsPage } from "@/components/dashboard/hosts/hosts-page";
import { TasksPage } from "@/components/dashboard/tasks/tasks-page";
import { ApprovalsPage } from "@/components/dashboard/approvals/approvals-page";
import { AppsPage } from "@/components/dashboard/apps/apps-page";
import { SettingsPage } from "@/components/dashboard/settings/settings-page";
import type {
  AppSummary,
  ApprovalRequest,
  Host,
  SettingsResponse,
  Task,
} from "@/types/api";

type DashboardData = {
  hosts: Host[];
  tasks: Task[];
  approvals: ApprovalRequest[];
  apps: AppSummary[];
  settings: SettingsResponse | null;
  error: string | null;
};

const emptyData: DashboardData = {
  hosts: [],
  tasks: [],
  approvals: [],
  apps: [],
  settings: null,
  error: null,
};

export function DashboardDataPage({ view }: { view: "overview" | "hosts" | "tasks" | "approvals" | "apps" | "settings" }) {
  const [data, setData] = useState<DashboardData>(emptyData);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      const [hosts, tasks, approvals, apps, settings] = await Promise.all([
        getHosts(),
        getTasks(),
        getApprovals(),
        getApps(),
        getSettings(),
      ]);
      if (cancelled) {
        return;
      }
      setData({
        hosts: hosts.data?.items ?? [],
        tasks: tasks.data?.items ?? [],
        approvals: approvals.data?.items ?? [],
        apps: apps.data?.items ?? [],
        settings: settings.data,
        error:
          hosts.error ??
          tasks.error ??
          approvals.error ??
          apps.error ??
          settings.error,
      });
    }

    load();

    return () => {
      cancelled = true;
    };
  }, []);

  if (view === "hosts") {
    return <HostsPage hosts={data.hosts} apiError={data.error} />;
  }
  if (view === "tasks") {
    return <TasksPage tasks={data.tasks} apiError={data.error} />;
  }
  if (view === "approvals") {
    return <ApprovalsPage approvals={data.approvals} apiError={data.error} />;
  }
  if (view === "apps") {
    return (
      <AppsPage
        initialApplications={data.apps.length > 0 ? toApplications(data.apps) : undefined}
        apiError={data.error}
      />
    );
  }
  if (view === "settings") {
    return <SettingsPage settings={data.settings} apiError={data.error} />;
  }

  return (
    <OverviewPage
      hosts={data.hosts}
      tasks={data.tasks}
      approvals={data.approvals}
      apps={data.apps}
      apiError={data.error}
    />
  );
}
