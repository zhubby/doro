import { PlaceholderPage } from "@/components/dashboard/placeholder-page";

export default function Ai() {
  return (
    <PlaceholderPage
      title="AI"
      description="承接智能体运行环境、工具调用和任务执行入口。"
      items={["智能体", "运行环境", "任务队列", "模型配置"]}
    />
  );
}
