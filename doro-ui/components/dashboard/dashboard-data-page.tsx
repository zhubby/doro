"use client";

import { useEffect, useRef, useState } from "react";

import {
  getApps,
  getApprovals,
  getControlPlaneEnvironment,
  getHostMetrics,
  getHosts,
  getSettings,
  getTasks,
  refreshVirtualMachines,
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
  ControlPlaneEnvironment,
  Host,
  MetricSnapshot,
  SettingsResponse,
  Task,
  VirtualMachine,
} from "@/types/api";

type DashboardData = {
  hosts: Host[];
  tasks: Task[];
  approvals: ApprovalRequest[];
  apps: AppSummary[];
  controlPlaneEnvironment: ControlPlaneEnvironment | null;
  metricHistoryByHost: Record<string, MetricSnapshot[]>;
  settings: SettingsResponse | null;
  virtualMachines: VirtualMachine[];
  error: string | null;
};

const emptyData: DashboardData = {
  hosts: [],
  tasks: [],
  approvals: [],
  apps: [],
  controlPlaneEnvironment: null,
  metricHistoryByHost: {},
  settings: null,
  virtualMachines: [],
  error: null,
};

const DASHBOARD_REFRESH_INTERVAL_MS = 10_000;
const DASHBOARD_METRIC_HISTORY_LIMIT = 240;

export function DashboardDataPage({ view }: { view: "overview" | "hosts" | "tasks" | "approvals" | "apps" | "settings" }) {
  const [data, setData] = useState<DashboardData>(emptyData);
  const controlPlaneEnvironmentLoaded = useRef(false);

  useEffect(() => {
    let cancelled = false;
    let refreshTimer: ReturnType<typeof setTimeout> | null = null;

    async function load() {
      const [hosts, tasks, approvals, apps, settings, virtualMachines] = await Promise.all([
        getHosts(),
        getTasks(),
        getApprovals(),
        getApps(),
        getSettings(),
        view === "apps"
          ? refreshVirtualMachines()
          : Promise.resolve({ data: null, error: null }),
      ]);
      if (cancelled) {
        return;
      }
      const hostItems = hosts.data?.items ?? [];
      const shouldLoadControlPlaneEnvironment =
        view === "overview" && !controlPlaneEnvironmentLoaded.current;
      const [metricResults, controlPlaneEnvironment] = await Promise.all([
        Promise.all(
          hostItems.map((host) => getHostMetrics(host.id, DASHBOARD_METRIC_HISTORY_LIMIT)),
        ),
        shouldLoadControlPlaneEnvironment
          ? getControlPlaneEnvironment()
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
        virtualMachines.error ??
        metricResults.find((result) => result.error)?.error ??
        controlPlaneEnvironment.error ??
        null;
      const metricHistoryByHost = Object.fromEntries(
        hostItems.map((host, index) => [
          host.id,
          metricResults[index]?.data?.items ?? [],
        ]),
      );
      if (controlPlaneEnvironment.data?.item) {
        controlPlaneEnvironmentLoaded.current = true;
      }

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
          controlPlaneEnvironment:
            controlPlaneEnvironment.data?.item ?? current.controlPlaneEnvironment,
          metricHistoryByHost,
          settings: settings.data,
          virtualMachines: virtualMachines.data?.items ?? current.virtualMachines,
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
    return (
      <ApprovalsPage
        approvals={data.approvals}
        apiError={data.error}
        onApprovalCreated={(approval) => {
          setData((current) => ({
            ...current,
            approvals: [approval, ...current.approvals],
          }));
        }}
        onApprovalDeleted={(approvalId) => {
          setData((current) => ({
            ...current,
            approvals: current.approvals.filter((approval) => approval.id !== approvalId),
          }));
        }}
        onApprovalUpdated={(approval) => {
          setData((current) => ({
            ...current,
            approvals: current.approvals.map((item) =>
              item.id === approval.id ? approval : item,
            ),
          }));
        }}
      />
    );
  }
  if (view === "apps") {
    return <AppsPage machines={data.virtualMachines} hosts={data.hosts} apiError={data.error} />;
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
      controlPlaneEnvironment={data.controlPlaneEnvironment}
      metricHistoryByHost={data.metricHistoryByHost}
      apiError={data.error}
    />
  );
}
