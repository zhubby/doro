import { PlaceholderPage } from "@/components/dashboard/placeholder-page";

export default function Tools() {
  return (
    <PlaceholderPage
      title="工具箱"
      description="用卡片网格呈现常用系统工具和诊断入口。"
      items={["进程管理", "网络诊断", "日志查看", "文件工具"]}
    />
  );
}
