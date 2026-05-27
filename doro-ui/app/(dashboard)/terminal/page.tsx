import { PlaceholderPage } from "@/components/dashboard/placeholder-page";

export default function Terminal() {
  return (
    <PlaceholderPage
      title="终端"
      description="先呈现终端入口和连接状态，后续再集成真实终端能力。"
      items={["本地终端", "远程连接", "会话记录"]}
    />
  );
}
