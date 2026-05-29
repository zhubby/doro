import type {
  AuthStatusResponse,
  AuthTokenResponse,
  ControlPlaneEnvironmentResponse,
  CreateEnrollmentTokenRequest,
  CreateEnrollmentTokenResponse,
  CurrentUserResponse,
  LatestMetricResponse,
  ListAppsResponse,
  ListApprovalsResponse,
  ListHostContainersResponse,
  ListHostsResponse,
  ListMetricSnapshotsResponse,
  ListRuntimeLogsResponse,
  ListTasksResponse,
  LoginRequest,
  RefreshTokenRequest,
  RegisterRequest,
  SettingsResponse,
  TerminalCommandRequest,
  TerminalCommandResponse,
  UpdateHostRequest,
  UpdateHostResponse,
} from "@/types/api";

const DEFAULT_CONTROL_PLANE_URL = "http://127.0.0.1:8787";
const AUTH_STORAGE_KEY = "doro-auth";
const REFRESH_SKEW_MS = 5 * 60 * 1000;

type ApiResult<T> = {
  data: T | null;
  error: string | null;
  status?: number;
};

type StoredAuth = {
  accessToken: string;
  refreshToken: string;
  expiresAt: string;
};

function controlPlaneUrl() {
  const configuredUrl =
    process.env.NEXT_PUBLIC_DORO_CONTROL_PLANE_URL ??
    process.env.DORO_CONTROL_PLANE_URL ??
    DEFAULT_CONTROL_PLANE_URL;

  return configuredUrl.replace(/\/$/, "");
}

function controlPlaneWebSocketUrl() {
  const url = controlPlaneUrl();
  return url.replace(/^http:/, "ws:").replace(/^https:/, "wss:");
}

function readAuth(): StoredAuth | null {
  if (typeof window === "undefined") {
    return null;
  }

  const raw = window.localStorage.getItem(AUTH_STORAGE_KEY);
  if (!raw) {
    return null;
  }

  try {
    return JSON.parse(raw) as StoredAuth;
  } catch {
    window.localStorage.removeItem(AUTH_STORAGE_KEY);
    return null;
  }
}

function writeAuth(response: AuthTokenResponse) {
  if (typeof window === "undefined") {
    return;
  }

  window.localStorage.setItem(
    AUTH_STORAGE_KEY,
    JSON.stringify({
      accessToken: response.access_token,
      refreshToken: response.refresh_token,
      expiresAt: response.expires_at,
    } satisfies StoredAuth),
  );
}

export function clearAuth() {
  if (typeof window === "undefined") {
    return;
  }
  window.localStorage.removeItem(AUTH_STORAGE_KEY);
}

function shouldRefresh(auth: StoredAuth) {
  return new Date(auth.expiresAt).getTime() - Date.now() < REFRESH_SKEW_MS;
}

async function requestJson<T>(
  path: string,
  init: RequestInit = {},
  token?: string,
): Promise<ApiResult<T>> {
  const headers = new Headers(init.headers);
  headers.set("Accept", "application/json");
  if (init.body) {
    headers.set("Content-Type", "application/json");
  }
  if (token) {
    headers.set("Authorization", `Bearer ${token}`);
  }

  try {
    const response = await fetch(`${controlPlaneUrl()}${path}`, {
      ...init,
      cache: "no-store",
      headers,
    });

    if (!response.ok) {
      return {
        data: null,
        error: `控制平面返回 ${response.status}`,
        status: response.status,
      };
    }

    if (response.status === 204) {
      return { data: null as T, error: null, status: response.status };
    }

    return {
      data: (await response.json()) as T,
      error: null,
      status: response.status,
    };
  } catch (error) {
    return {
      data: null,
      error: error instanceof Error ? error.message : "无法连接控制平面",
    };
  }
}

async function refreshAuth(auth: StoredAuth): Promise<StoredAuth | null> {
  const result = await requestJson<AuthTokenResponse>("/api/v1/auth/refresh", {
    method: "POST",
    body: JSON.stringify({
      refresh_token: auth.refreshToken,
    } satisfies RefreshTokenRequest),
  });

  if (!result.data) {
    clearAuth();
    return null;
  }

  writeAuth(result.data);
  return {
    accessToken: result.data.access_token,
    refreshToken: result.data.refresh_token,
    expiresAt: result.data.expires_at,
  };
}

async function authToken() {
  let auth = readAuth();
  if (!auth) {
    return null;
  }

  if (shouldRefresh(auth)) {
    auth = await refreshAuth(auth);
  }

  return auth?.accessToken ?? null;
}

async function getJson<T>(path: string): Promise<ApiResult<T>> {
  let token = await authToken();
  if (!token) {
    return { data: null, error: "未登录", status: 401 };
  }

  let result = await requestJson<T>(path, {}, token);
  if (result.status === 401) {
    const current = readAuth();
    const refreshed = current ? await refreshAuth(current) : null;
    token = refreshed?.accessToken ?? null;
    result = token ? await requestJson<T>(path, {}, token) : result;
  }

  return result;
}

async function authedRequest<T>(
  path: string,
  init: RequestInit = {},
): Promise<ApiResult<T>> {
  let token = await authToken();
  if (!token) {
    return { data: null, error: "未登录", status: 401 };
  }

  let result = await requestJson<T>(path, init, token);
  if (result.status === 401) {
    const current = readAuth();
    const refreshed = current ? await refreshAuth(current) : null;
    token = refreshed?.accessToken ?? null;
    result = token ? await requestJson<T>(path, init, token) : result;
  }

  return result;
}

export async function authStatus() {
  return requestJson<AuthStatusResponse>("/api/v1/auth/status");
}

export async function login(request: LoginRequest) {
  const result = await requestJson<AuthTokenResponse>("/api/v1/auth/login", {
    method: "POST",
    body: JSON.stringify(request),
  });
  if (result.data) {
    writeAuth(result.data);
  }
  return result;
}

export async function register(request: RegisterRequest) {
  const result = await requestJson<AuthTokenResponse>("/api/v1/auth/register", {
    method: "POST",
    body: JSON.stringify(request),
  });
  if (result.data) {
    writeAuth(result.data);
  }
  return result;
}

export async function logout() {
  const auth = readAuth();
  if (auth) {
    await requestJson("/api/v1/auth/logout", {
      method: "POST",
      body: JSON.stringify({
        refresh_token: auth.refreshToken,
      } satisfies RefreshTokenRequest),
    }, auth.accessToken);
  }
  clearAuth();
}

export async function currentUser() {
  return getJson<CurrentUserResponse>("/api/v1/auth/me");
}

export async function getHosts() {
  return getJson<ListHostsResponse>("/api/v1/hosts");
}

export async function createEnrollmentToken(
  request: CreateEnrollmentTokenRequest = { label: null },
) {
  return authedRequest<CreateEnrollmentTokenResponse>(
    "/api/v1/hosts/enrollment-token",
    {
      method: "POST",
      body: JSON.stringify(request),
    },
  );
}

export async function deleteHost(hostId: string) {
  return authedRequest<null>(`/api/v1/hosts/${hostId}`, {
    method: "DELETE",
  });
}

export async function updateHost(hostId: string, request: UpdateHostRequest) {
  return authedRequest<UpdateHostResponse>(`/api/v1/hosts/${hostId}`, {
    method: "PATCH",
    body: JSON.stringify(request),
  });
}

export async function getLatestHostMetric(hostId: string) {
  return getJson<LatestMetricResponse>(`/api/v1/hosts/${hostId}/metrics/latest`);
}

export async function getHostMetrics(hostId: string, limit = 60) {
  return getJson<ListMetricSnapshotsResponse>(
    `/api/v1/hosts/${hostId}/metrics?limit=${limit}`,
  );
}

export async function getHostContainers(hostId: string) {
  return getJson<ListHostContainersResponse>(`/api/v1/hosts/${hostId}/containers`);
}

export async function refreshContainers() {
  return getJson<ListHostContainersResponse>("/api/v1/containers");
}

export async function getControlPlaneEnvironment() {
  return getJson<ControlPlaneEnvironmentResponse>("/api/v1/control-plane/environment");
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

export async function getControlPlaneLogs(limit = 500) {
  return getJson<ListRuntimeLogsResponse>(
    `/api/v1/logs/control-plane?limit=${limit}`,
  );
}

export async function getAgentLogs(hostId: string, limit = 500) {
  return getJson<ListRuntimeLogsResponse>(
    `/api/v1/logs/agents/${hostId}?limit=${limit}`,
  );
}

export async function runtimeLogStreamUrl(
  scope: "control_plane" | "agent",
  hostId?: string,
) {
  const token = await authToken();
  if (!token) {
    return null;
  }
  const query = new URLSearchParams({
    scope,
    token,
  });
  if (hostId) {
    query.set("host_id", hostId);
  }
  return `${controlPlaneUrl()}/api/v1/logs/stream?${query}`;
}

export async function runTerminalCommand(request: TerminalCommandRequest) {
  return authedRequest<TerminalCommandResponse>("/api/v1/terminal/commands", {
    method: "POST",
    body: JSON.stringify(request),
  });
}

export async function terminalSessionWebSocketUrl(hostId: string, cols: number, rows: number) {
  const token = await authToken();
  if (!token) {
    return null;
  }
  const query = new URLSearchParams({
    token,
    cols: String(cols),
    rows: String(rows),
  });
  return `${controlPlaneWebSocketUrl()}/api/v1/terminal/${hostId}/ws?${query}`;
}
