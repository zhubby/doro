import { PlaceholderPage } from "@/components/dashboard/placeholder-page";

export default function System() {
  return (
    <PlaceholderPage
      title="系统"
      description="扩展首页系统状态，呈现主机、磁盘、网络和安全信息。"
      items={["主机信息", "磁盘容量", "网络流量", "安全巡检"]}
    />
  );
}
