import type {
  ListAppsResponse,
  ListApprovalsResponse,
  ListHostsResponse,
  ListTasksResponse,
  SettingsResponse,
} from "@/types/api";

const DEFAULT_CONTROL_PLANE_URL = "http://127.0.0.1:8787";

type ApiResult<T> = {
  data: T | null;
  error: string | null;
};

function controlPlaneUrl() {
  const configuredUrl =
    process.env.NEXT_PUBLIC_DORO_CONTROL_PLANE_URL ??
    process.env.DORO_CONTROL_PLANE_URL ??
    DEFAULT_CONTROL_PLANE_URL;

  return configuredUrl.replace(/\/$/, "");
}

async function getJson<T>(path: string): Promise<ApiResult<T>> {
  const url = `${controlPlaneUrl()}${path}`;

  try {
    const response = await fetch(url, {
      cache: "no-store",
      headers: {
        Accept: "application/json",
      },
    });

    if (!response.ok) {
      return {
        data: null,
        error: `控制平面返回 ${response.status}`,
      };
    }

    return {
      data: (await response.json()) as T,
      error: null,
    };
  } catch (error) {
    return {
      data: null,
      error: error instanceof Error ? error.message : "无法连接控制平面",
    };
  }
}

export async function getHosts() {
  return getJson<ListHostsResponse>("/api/v1/hosts");
}

export async function getTasks() {
  return getJson<ListTasksResponse>("/api/v1/tasks");
}

export async function getApprovals() {
  return getJson<ListApprovalsResponse>("/api/v1/approvals");
}

export async function getApps() {
  return getJson<ListAppsResponse>("/api/v1/apps");
}

export async function getSettings() {
  return getJson<SettingsResponse>("/api/v1/settings");
}

