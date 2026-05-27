import { PlaceholderPage } from "@/components/dashboard/placeholder-page";

export default function Websites() {
  return (
    <PlaceholderPage
      title="网站"
      description="使用列表页模式承接网站、域名、SSL 和运行状态。"
      items={["站点列表", "域名绑定", "SSL 证书", "反向代理"]}
    />
  );
}
