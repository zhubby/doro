import { PlaceholderPage } from "@/components/dashboard/placeholder-page";

export default function ApprovalsRoute() {
  return (
    <PlaceholderPage
      title="审批"
      description="处理 shell、文件写入、容器删除、端口暴露等高风险操作。"
      items={["待审批操作", "风险说明", "审批历史", "策略命中记录"]}
    />
  );
}
