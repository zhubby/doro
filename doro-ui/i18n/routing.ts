import { defineRouting } from "next-intl/routing";

export const locales = ["zh-CN", "en-US"] as const;
export const defaultLocale = "zh-CN";

export type AppLocale = (typeof locales)[number];

export const routing = defineRouting({
  locales,
  defaultLocale,
  localePrefix: "as-needed",
  localeCookie: {
    name: "doro-locale",
    sameSite: "lax",
  },
});

export function isAppLocale(value: string): value is AppLocale {
  return locales.includes(value as AppLocale);
}
