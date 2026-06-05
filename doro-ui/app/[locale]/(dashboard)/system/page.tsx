"use client";

import { useEffect, useState } from "react";

import { SystemPage } from "@/components/dashboard/system/system-page";
import { getHostMetrics, getHosts } from "@/lib/control-plane-api";
import type { Host, MetricSnapshot } from "@/types/api";

const SYSTEM_REFRESH_INTERVAL_MS = 10_000;
const SYSTEM_METRIC_HISTORY_LIMIT = 240;

function defaultHostId(hosts: Host[]) {
  return hosts.find((host) => host.status === "online")?.id ?? hosts[0]?.id ?? "";
}

export default function System() {
  const [hosts, setHosts] = useState<Host[]>([]);
  const [selectedHostId, setSelectedHostId] = useState("");
  const [metricHistory, setMetricHistory] = useState<MetricSnapshot[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    let refreshTimer: ReturnType<typeof setTimeout> | null = null;

    async function load() {
      const hostsResult = await getHosts();
      const hostItems = hostsResult.data?.items ?? [];
      const targetHostId =
        selectedHostId && hostItems.some((host) => host.id === selectedHostId)
          ? selectedHostId
          : defaultHostId(hostItems);
      const metricResult = targetHostId
        ? await getHostMetrics(targetHostId, SYSTEM_METRIC_HISTORY_LIMIT)
        : { data: null, error: null };

      if (cancelled) {
        return;
      }
      setHosts(hostItems);
      setSelectedHostId(targetHostId);
      setMetricHistory(metricResult.data?.items ?? []);
      setError(hostsResult.error ?? metricResult.error);
    }

    async function refresh() {
      await load();
      if (!cancelled) {
        refreshTimer = setTimeout(refresh, SYSTEM_REFRESH_INTERVAL_MS);
      }
    }

    refresh();

    return () => {
      cancelled = true;
      if (refreshTimer) {
        clearTimeout(refreshTimer);
      }
    };
  }, [selectedHostId]);

  return (
    <SystemPage
      hosts={hosts}
      selectedHostId={selectedHostId}
      metricHistory={metricHistory}
      apiError={error}
      onSelectedHostChange={setSelectedHostId}
    />
  );
}
