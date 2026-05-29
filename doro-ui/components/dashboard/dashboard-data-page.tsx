"use client";

import { useEffect, useState } from "react";

import {
  getApps,
  getApprovals,
  getHostMetrics,
  getHosts,
  getSettings,
  getTasks,
  refreshContainers,
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
  HostContainer,
  MetricSnapshot,
  SettingsResponse,
  Task,
} from "@/types/api";

type DashboardData = {
  hosts: Host[];
  tasks: Task[];
  approvals: ApprovalRequest[];
  apps: AppSummary[];
  containers: HostContainer[];
  metricHistoryByHost: Record<string, MetricSnapshot[]>;
  settings: SettingsResponse | null;
  error: string | null;
};

const emptyData: DashboardData = {
  hosts: [],
  tasks: [],
  approvals: [],
  apps: [],
  containers: [],
  metricHistoryByHost: {},
  settings: null,
  error: null,
};

const DASHBOARD_REFRESH_INTERVAL_MS = 10_000;
const DASHBOARD_METRIC_HISTORY_LIMIT = 240;

export function DashboardDataPage({ view }: { view: "overview" | "hosts" | "tasks" | "approvals" | "apps" | "settings" }) {
  const [data, setData] = useState<DashboardData>(emptyData);

  useEffect(() => {
    let cancelled = false;
    let refreshTimer: ReturnType<typeof setTimeout> | null = null;

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
      const hostItems = hosts.data?.items ?? [];
      const [metricResults, containers] = await Promise.all([
        Promise.all(
          hostItems.map((host) => getHostMetrics(host.id, DASHBOARD_METRIC_HISTORY_LIMIT)),
        ),
        view === "overview"
          ? refreshContainers()
          : Promise.resolve({ data: null, error: null }),
      ]);
      if (cancelled) {
        return;
      }
      const error =
        hosts.error ??
        tasks.error ??
        approvals.error ??
        apps.error ??
        settings.error ??
        metricResults.find((result) => result.error)?.error ??
        containers.error ??
        null;
      const metricHistoryByHost = Object.fromEntries(
        hostItems.map((host, index) => [
          host.id,
          metricResults[index]?.data?.items ?? [],
        ]),
      );

      setData((current) => {
        if (error) {
          return {
            ...current,
            error,
          };
        }

        return {
          hosts: hostItems,
          tasks: tasks.data?.items ?? [],
          approvals: approvals.data?.items ?? [],
          apps: apps.data?.items ?? [],
          containers: containers.data?.items ?? current.containers,
          metricHistoryByHost,
          settings: settings.data,
          error: null,
        };
      });
    }

    async function refresh() {
      await load();
      if (!cancelled) {
        refreshTimer = setTimeout(refresh, DASHBOARD_REFRESH_INTERVAL_MS);
      }
    }

    refresh();

    return () => {
      cancelled = true;
      if (refreshTimer) {
        clearTimeout(refreshTimer);
      }
    };
  }, []);

  if (view === "hosts") {
    return (
      <HostsPage
        hosts={data.hosts}
        metricHistoryByHost={data.metricHistoryByHost}
        apiError={data.error}
        onHostDeleted={(hostId) => {
          setData((current) => {
            const metricHistoryByHost = { ...current.metricHistoryByHost };
            delete metricHistoryByHost[hostId];
            return {
              ...current,
              hosts: current.hosts.filter((host) => host.id !== hostId),
              metricHistoryByHost,
            };
          });
        }}
        onHostUpdated={(host) => {
          setData((current) => ({
            ...current,
            hosts: current.hosts.map((item) =>
              item.id === host.id ? host : item,
            ),
          }));
        }}
      />
    );
  }
  if (view === "tasks") {
    return <TasksPage tasks={data.tasks} apiError={data.error} />;
  }
  if (view === "approvals") {
    return <ApprovalsPage approvals={data.approvals} apiError={data.error} />;
  }
  if (view === "apps") {
    return <AppsPage apiError={data.error} />;
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
      containers={data.containers}
      metricHistoryByHost={data.metricHistoryByHost}
      apiError={data.error}
    />
  );
}
