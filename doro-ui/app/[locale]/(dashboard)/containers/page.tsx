"use client";

import { useCallback, useEffect, useState } from "react";

import { ContainersPage } from "@/components/dashboard/resources/containers-page";
import {
  getHostContainers,
  getHosts,
  refreshContainers,
} from "@/lib/control-plane-api";
import type { Host, HostContainer } from "@/types/api";

export default function Containers() {
  const [hosts, setHosts] = useState<Host[]>([]);
  const [containers, setContainers] = useState<HostContainer[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [isRefreshing, setIsRefreshing] = useState(true);

  const loadContainers = useCallback(async (options: { signal?: AbortSignal } = {}) => {
    const { signal } = options;
    try {
      setIsRefreshing(true);
      const hostsResult = await getHosts();
      if (signal?.aborted) {
        return;
      }
      const hostItems = hostsResult.data?.items ?? [];
      const refreshResult = hostsResult.data ? await refreshContainers() : null;
      if (signal?.aborted) {
        return;
      }
      const containerResults =
        refreshResult?.data ?
          []
        : await Promise.all(hostItems.map((host) => getHostContainers(host.id)));
      if (signal?.aborted) {
        return;
      }
      setHosts(hostItems);
      setContainers(
        refreshResult?.data?.items ??
          containerResults.flatMap((result) => result.data?.items ?? []),
      );
      setError(
        hostsResult.error ??
          refreshResult?.error ??
          containerResults.find((result) => result.error)?.error ??
          null,
      );
      setIsRefreshing(false);
    } catch (error: unknown) {
      if (signal?.aborted) {
        return;
      }
      setError(error instanceof Error ? error.message : "无法加载容器数据");
      setIsRefreshing(false);
    }
  }, []);

  useEffect(() => {
    const abortController = new AbortController();

    void loadContainers({ signal: abortController.signal });

    return () => {
      abortController.abort();
    };
  }, [loadContainers]);

  return (
    <ContainersPage
      hosts={hosts}
      containers={containers}
      apiError={error}
      isRefreshing={isRefreshing}
      onRefresh={() => {
        void loadContainers();
      }}
    />
  );
}
