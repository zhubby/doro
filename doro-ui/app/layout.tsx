import type { Metadata } from "next";
import "./globals.css";

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

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="zh-CN" suppressHydrationWarning>
      <body>
        <script dangerouslySetInnerHTML={{ __html: themeScript }} />
        {children}
      </body>
    </html>
  );
}
