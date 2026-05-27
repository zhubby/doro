import { PlaceholderPage } from "@/components/dashboard/placeholder-page";

export default function LogsRoute() {
  return (
    <PlaceholderPage
      title="日志"
      description="集中查看控制面、Agent、任务和应用运行日志。"
      items={["控制面日志", "Agent 日志", "任务事件", "应用日志"]}
    />
  );
}
