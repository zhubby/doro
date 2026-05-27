import { PlaceholderPage } from "@/components/dashboard/placeholder-page";

export default function Cron() {
  return (
    <PlaceholderPage
      title="计划任务"
      description="按 1Panel 任务列表模式展示备份、巡检和自动化任务。"
      items={["备份任务", "巡检任务", "执行日志", "失败重试"]}
    />
  );
}
