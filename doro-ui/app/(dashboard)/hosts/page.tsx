import { PlaceholderPage } from "@/components/dashboard/placeholder-page";

export default function HostsRoute() {
  return (
    <PlaceholderPage
      title="主机"
      description="查看 Agent 注册状态、能力声明、心跳和主机资源概览。"
      items={["Agent 列表", "能力声明", "心跳状态", "主机标签"]}
    />
  );
}
