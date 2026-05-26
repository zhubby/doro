"use client";

import { useEffect, useMemo, useState, type ElementType } from "react";
import {
  Activity,
  AppWindow,
  Boxes,
  CheckCircle2,
  CircleGauge,
  Database,
  Gauge,
  HardDrive,
  Home,
  Layers3,
  Moon,
  Network,
  NotebookPen,
  Search,
  Server,
  Settings,
  ShieldCheck,
  SquareTerminal,
  Sun,
  Wrench,
  Zap,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { cn } from "@/lib/utils";

type NavigationItem = {
  label: string;
  icon: ElementType;
  count?: number;
};

type AppState = "installed" | "available" | "running";
type ThemeMode = "light" | "dark";

const navigation: NavigationItem[] = [
  { label: "概览", icon: Home },
  { label: "应用商店", icon: AppWindow, count: 6 },
  { label: "AI", icon: Zap },
  { label: "网站", icon: Network, count: 1 },
  { label: "数据库", icon: Database, count: 2 },
  { label: "容器", icon: Boxes },
  { label: "系统", icon: Server },
  { label: "终端", icon: SquareTerminal },
  { label: "计划任务", icon: CheckCircle2 },
  { label: "工具箱", icon: Wrench },
  { label: "面板设置", icon: Settings },
];

const overviewStats = [
  { label: "智能体", value: "0", helper: "待接入" },
  { label: "网站", value: "1", helper: "1 个运行中" },
  { label: "数据库", value: "2", helper: "MySQL / Redis" },
  { label: "已安装应用", value: "6", helper: "2 个可更新" },
];

const systemStats = [
  { label: "负载", value: "4.83%", detail: "运行流畅", progress: 5 },
  { label: "CPU", value: "4.23%", detail: "0.17 / 4 核", progress: 4 },
  { label: "内存", value: "17.24%", detail: "1.29 GB / 7.31 GB", progress: 17 },
  { label: "磁盘", value: "17.91%", detail: "33.2 GB / 186.49 GB", progress: 18 },
];

const trafficMetrics = [
  { label: "上行", value: "4.02 KB/s" },
  { label: "下行", value: "8.66 KB/s" },
  { label: "总发送", value: "1.06 TB" },
  { label: "总接收", value: "139.65 GB" },
];

const diskMetrics = [
  { label: "读取", value: "18.4 MB/s" },
  { label: "写入", value: "6.8 MB/s" },
  { label: "IO 使用率", value: "12%" },
  { label: "等待", value: "2 ms" },
];

const trendBars = [18, 26, 22, 34, 28, 44, 38, 52, 46, 58, 48, 64];

const notes = [
  "本周检查应用更新与备份策略。",
  "开启安全入口后再暴露公网访问。",
  "数据库监控阈值保持默认配置。",
];

const initialApplications: Record<string, AppState> = {
  openresty: "running",
  mysql: "installed",
  redis: "available",
  hermes: "available",
};

const applications = [
  {
    id: "openresty",
    name: "OpenResty",
    version: "1.27.1.2-0.1-local",
    description: "高性能 Web 服务运行环境",
    icon: Network,
  },
  {
    id: "mysql",
    name: "MySQL",
    version: "8.0.37",
    description: "关系型数据库服务",
    icon: Database,
  },
  {
    id: "redis",
    name: "Redis",
    version: "7.2.5",
    description: "内存键值缓存服务",
    icon: Layers3,
  },
  {
    id: "hermes",
    name: "Hermes Agent",
    version: "0.4.1",
    description: "智能体调度与任务执行",
    icon: Activity,
  },
];

function getApplicationAction(state: AppState) {
  if (state === "running") {
    return "管理";
  }

  if (state === "installed") {
    return "启动";
  }

  return "安装";
}

function getNextApplicationState(state: AppState): AppState {
  if (state === "available") {
    return "installed";
  }

  return "running";
}

export function ControlPanel() {
  const [activeSection, setActiveSection] = useState("概览");
  const [theme, setTheme] = useState<ThemeMode>("light");
  const [applicationStates, setApplicationStates] =
    useState<Record<string, AppState>>(initialApplications);

  useEffect(() => {
    const storedTheme = window.localStorage.getItem("doro-theme");
    const systemTheme = window.matchMedia("(prefers-color-scheme: dark)")
      .matches
      ? "dark"
      : "light";
    const nextTheme =
      storedTheme === "light" || storedTheme === "dark"
        ? storedTheme
        : systemTheme;

    setTheme(nextTheme);
  }, []);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", theme === "dark");
    window.localStorage.setItem("doro-theme", theme);
  }, [theme]);

  const activeSummary = useMemo(
    () => navigation.find((item) => item.label === activeSection),
    [activeSection],
  );

  const isDark = theme === "dark";

  return (
    <div className="min-h-screen bg-background text-foreground">
      <div className="grid min-h-screen lg:grid-cols-[17rem_1fr]">
        <aside className="border-b bg-card lg:border-b-0 lg:border-r">
          <div className="flex h-full flex-col">
            <div className="flex h-16 items-center gap-3 px-6">
              <div className="flex size-9 items-center justify-center rounded-lg bg-primary text-primary-foreground">
                <Boxes className="size-4" aria-hidden="true" />
              </div>
              <div>
                <p className="text-sm font-semibold">Doro Panel</p>
                <p className="text-xs text-muted-foreground">本地控制台</p>
              </div>
            </div>
            <Separator />
            <ScrollArea className="flex-1 px-3 py-4">
              <nav className="grid gap-1" aria-label="控制面板导航">
                {navigation.map((item) => {
                  const Icon = item.icon;
                  const isActive = activeSection === item.label;

                  return (
                    <Button
                      key={item.label}
                      type="button"
                      variant={isActive ? "secondary" : "ghost"}
                      className="justify-start"
                      onClick={() => setActiveSection(item.label)}
                    >
                      <Icon className="size-4" aria-hidden="true" />
                      <span>{item.label}</span>
                      {item.count ? (
                        <Badge variant="outline" className="ml-auto">
                          {item.count}
                        </Badge>
                      ) : null}
                    </Button>
                  );
                })}
              </nav>
            </ScrollArea>
            <Separator />
            <div className="p-4">
              <Card className="shadow-none">
                <CardHeader className="p-4 pb-2">
                  <CardTitle className="text-sm">入口状态</CardTitle>
                  <CardDescription>安全入口已启用</CardDescription>
                </CardHeader>
                <CardContent className="flex items-center gap-2 p-4 pt-0">
                  <ShieldCheck
                    className="size-4 text-primary"
                    aria-hidden="true"
                  />
                  <span className="text-xs text-muted-foreground">
                    v2.1.13-alpha.2
                  </span>
                </CardContent>
              </Card>
            </div>
          </div>
        </aside>

        <main className="flex min-w-0 flex-col">
          <header className="flex min-h-16 flex-col gap-3 border-b px-6 py-4 md:flex-row md:items-center md:justify-between">
            <div>
              <div className="flex items-center gap-2">
                <h1 className="text-2xl font-semibold tracking-tight">
                  {activeSection}
                </h1>
                {activeSummary?.count ? (
                  <Badge variant="secondary">{activeSummary.count}</Badge>
                ) : null}
              </div>
              <p className="text-sm text-muted-foreground">
                统一查看站点、数据库、应用和服务器运行状态。
              </p>
            </div>
            <div className="flex flex-wrap gap-2">
              <Button variant="outline">
                <Search className="size-4" aria-hidden="true" />
                搜索
              </Button>
              <Button
                variant="outline"
                onClick={() => setTheme(isDark ? "light" : "dark")}
              >
                {isDark ? (
                  <Sun className="size-4" aria-hidden="true" />
                ) : (
                  <Moon className="size-4" aria-hidden="true" />
                )}
                {isDark ? "浅色" : "深色"}
              </Button>
              <Button>
                <Gauge className="size-4" aria-hidden="true" />
                快速巡检
              </Button>
            </div>
          </header>

          <div className="grid flex-1 gap-6 p-6 xl:grid-cols-[1fr_22rem]">
            <section className="min-w-0 space-y-6">
              <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
                {overviewStats.map((stat) => (
                  <Card key={stat.label}>
                    <CardHeader className="pb-2">
                      <CardDescription>{stat.label}</CardDescription>
                      <CardTitle className="text-3xl">{stat.value}</CardTitle>
                    </CardHeader>
                    <CardContent>
                      <p className="text-sm text-muted-foreground">
                        {stat.helper}
                      </p>
                    </CardContent>
                  </Card>
                ))}
              </div>

              <Card>
                <CardHeader>
                  <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
                    <div>
                      <CardTitle>系统状态</CardTitle>
                      <CardDescription>
                        关键资源使用率与容量概览
                      </CardDescription>
                    </div>
                    <Badge variant="outline">运行正常</Badge>
                  </div>
                </CardHeader>
                <CardContent className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
                  {systemStats.map((stat) => (
                    <div key={stat.label} className="rounded-lg border p-4">
                      <div className="mb-4 flex items-center justify-between">
                        <div className="flex items-center gap-2">
                          <CircleGauge
                            className="size-4 text-muted-foreground"
                            aria-hidden="true"
                          />
                          <span className="text-sm font-medium">
                            {stat.label}
                          </span>
                        </div>
                        <span className="text-sm font-semibold">
                          {stat.value}
                        </span>
                      </div>
                      <Progress value={stat.progress} />
                      <p className="mt-3 text-xs text-muted-foreground">
                        {stat.detail}
                      </p>
                    </div>
                  ))}
                </CardContent>
              </Card>

              <Card>
                <CardHeader>
                  <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
                    <div>
                      <CardTitle>监控</CardTitle>
                      <CardDescription>
                        使用 mock 数据展示流量和磁盘 IO 趋势
                      </CardDescription>
                    </div>
                    <Badge variant="secondary">近 1 小时</Badge>
                  </div>
                </CardHeader>
                <CardContent>
                  <Tabs defaultValue="traffic">
                    <TabsList>
                      <TabsTrigger value="traffic">
                        <Network className="mr-2 size-4" aria-hidden="true" />
                        流量
                      </TabsTrigger>
                      <TabsTrigger value="disk">
                        <HardDrive
                          className="mr-2 size-4"
                          aria-hidden="true"
                        />
                        磁盘 IO
                      </TabsTrigger>
                    </TabsList>
                    <TabsContent value="traffic" className="space-y-6">
                      <MetricGrid metrics={trafficMetrics} />
                      <TrendPreview label="网络吞吐趋势" />
                    </TabsContent>
                    <TabsContent value="disk" className="space-y-6">
                      <MetricGrid metrics={diskMetrics} />
                      <TrendPreview label="磁盘读写趋势" />
                    </TabsContent>
                  </Tabs>
                </CardContent>
              </Card>
            </section>

            <aside className="space-y-6">
              <Card>
                <CardHeader>
                  <div className="flex items-center justify-between">
                    <div>
                      <CardTitle>备忘录</CardTitle>
                      <CardDescription>运维提醒与待办记录</CardDescription>
                    </div>
                    <Button size="icon" variant="outline" aria-label="添加备忘">
                      <NotebookPen className="size-4" aria-hidden="true" />
                    </Button>
                  </div>
                </CardHeader>
                <CardContent className="space-y-3">
                  {notes.map((note) => (
                    <div key={note} className="rounded-lg border p-3 text-sm">
                      {note}
                    </div>
                  ))}
                </CardContent>
              </Card>

              <Card>
                <CardHeader>
                  <div className="flex items-center justify-between">
                    <div>
                      <CardTitle>应用</CardTitle>
                      <CardDescription>常用服务与安装状态</CardDescription>
                    </div>
                    <Button size="sm" variant="outline">
                      全部
                    </Button>
                  </div>
                </CardHeader>
                <CardContent>
                  <div className="space-y-4">
                    {applications.map((application) => {
                      const Icon = application.icon;
                      const state = applicationStates[application.id];

                      return (
                        <div
                          key={application.id}
                          className="flex items-center gap-3 rounded-lg border p-3"
                        >
                          <div className="flex size-10 items-center justify-center rounded-md bg-muted">
                            <Icon
                              className="size-5 text-muted-foreground"
                              aria-hidden="true"
                            />
                          </div>
                          <div className="min-w-0 flex-1">
                            <div className="flex items-center gap-2">
                              <p className="truncate text-sm font-medium">
                                {application.name}
                              </p>
                              <StatusBadge state={state} />
                            </div>
                            <p className="truncate text-xs text-muted-foreground">
                              {application.version}
                            </p>
                            <p className="truncate text-xs text-muted-foreground">
                              {application.description}
                            </p>
                          </div>
                          <Button
                            size="sm"
                            variant={state === "running" ? "outline" : "default"}
                            onClick={() =>
                              setApplicationStates((current) => ({
                                ...current,
                                [application.id]: getNextApplicationState(state),
                              }))
                            }
                          >
                            {getApplicationAction(state)}
                          </Button>
                        </div>
                      );
                    })}
                  </div>
                </CardContent>
              </Card>
            </aside>
          </div>
        </main>
      </div>
    </div>
  );
}

function MetricGrid({
  metrics,
}: {
  metrics: Array<{ label: string; value: string }>;
}) {
  return (
    <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
      {metrics.map((metric) => (
        <div key={metric.label} className="rounded-lg border p-3">
          <p className="text-xs text-muted-foreground">{metric.label}</p>
          <p className="mt-1 text-sm font-semibold">{metric.value}</p>
        </div>
      ))}
    </div>
  );
}

function TrendPreview({ label }: { label: string }) {
  return (
    <div className="rounded-lg border p-4">
      <div className="mb-4 flex items-center justify-between">
        <p className="text-sm font-medium">{label}</p>
        <div className="flex items-center gap-4 text-xs text-muted-foreground">
          <span>上行</span>
          <span>下行</span>
        </div>
      </div>
      <div className="flex h-40 items-end gap-2">
        {trendBars.map((height, index) => (
          <div
            key={`${height}-${index}`}
            className="flex flex-1 items-end rounded-md bg-muted"
          >
            <div
              className={cn(
                "w-full rounded-md bg-primary",
                index % 3 === 0 && "bg-primary/70",
              )}
              style={{ height: `${height}%` }}
            />
          </div>
        ))}
      </div>
    </div>
  );
}

function StatusBadge({ state }: { state: AppState }) {
  if (state === "running") {
    return <Badge>运行中</Badge>;
  }

  if (state === "installed") {
    return <Badge variant="secondary">已安装</Badge>;
  }

  return <Badge variant="outline">可安装</Badge>;
}
