import { getRequestConfig } from "next-intl/server";

import { defaultLocale, isAppLocale } from "@/i18n/routing";

type Messages = Record<string, unknown>;

const namespaces = [
  "common",
  "navigation",
  "auth",
  "dashboard",
  "resources",
  "settings",
] as const;

async function loadMessages(locale: string) {
  const entries = await Promise.all(
    namespaces.map(async (namespace) => [
      namespace,
      (await import(`../messages/${locale}/${namespace}.json`)).default,
    ]),
  );

  return Object.fromEntries(entries) as Messages;
}

export default getRequestConfig(async ({ requestLocale }) => {
  const requested = await requestLocale;
  const locale = requested && isAppLocale(requested) ? requested : defaultLocale;

  return {
    locale,
    messages: await loadMessages(locale),
  };
});
