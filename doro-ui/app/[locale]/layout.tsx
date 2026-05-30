import type { Metadata } from "next";
import { NextIntlClientProvider } from "next-intl";
import { setRequestLocale } from "next-intl/server";
import { notFound } from "next/navigation";

import { LocaleHtmlSync } from "@/components/layout/locale-html-sync";
import { isAppLocale, locales } from "@/i18n/routing";
import "../globals.css";

const themeScript = `
(() => {
  try {
    const storedTheme = window.localStorage.getItem("doro-theme");
    const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
    const theme = storedTheme === "light" || storedTheme === "dark"
      ? storedTheme
      : prefersDark
        ? "dark"
        : "light";
    document.documentElement.classList.toggle("dark", theme === "dark");
  } catch (_) {}
})();
`;

export const metadata: Metadata = {
  title: "Doro",
  description: "Doro frontend workspace",
};

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export default async function LocaleLayout({
  children,
  params,
}: Readonly<{
  children: React.ReactNode;
  params: Promise<{ locale: string }>;
}>) {
  const { locale } = await params;

  if (!isAppLocale(locale)) {
    notFound();
  }

  setRequestLocale(locale);

  return (
    <html lang={locale} suppressHydrationWarning>
      <body>
        <script dangerouslySetInnerHTML={{ __html: themeScript }} />
        <NextIntlClientProvider>
          <LocaleHtmlSync locale={locale} />
          {children}
        </NextIntlClientProvider>
      </body>
    </html>
  );
}
