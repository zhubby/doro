import { PlaceholderPage } from "@/components/dashboard/placeholder-page";

export default function TasksRoute() {
  return (
    <PlaceholderPage
      title="任务"
      description="跟踪控制面下发的任务、步骤、状态和执行结果。"
      items={["任务队列", "执行步骤", "失败重试", "AI 计划草稿"]}
    />
  );
}
