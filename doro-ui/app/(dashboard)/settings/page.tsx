import { SettingsPage } from "@/components/dashboard/settings/settings-page";
import { getSettings } from "@/lib/control-plane-api";

export const dynamic = "force-dynamic";

export default async function Settings() {
  const settings = await getSettings();

  return (
    <SettingsPage settings={settings.data} apiError={settings.error} />
  );
}
