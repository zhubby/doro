"use client";

import { useEffect, useState } from "react";

import type { ThemeMode } from "@/types/dashboard";

const storageKey = "doro-theme";

export function useTheme() {
  const [theme, setTheme] = useState<ThemeMode | null>(null);

  useEffect(() => {
    const storedTheme = window.localStorage.getItem(storageKey);
    const systemTheme = window.matchMedia("(prefers-color-scheme: dark)")
      .matches
      ? "dark"
      : "light";
    const nextTheme =
      storedTheme === "light" || storedTheme === "dark"
        ? storedTheme
        : systemTheme;

    setTheme(nextTheme);
  }, []);

  useEffect(() => {
    if (!theme) {
      return;
    }

    document.documentElement.classList.toggle("dark", theme === "dark");
    window.localStorage.setItem(storageKey, theme);
  }, [theme]);

  return {
    theme,
    isDark: theme === "dark",
    toggleTheme: () =>
      setTheme((current) => (current === "dark" ? "light" : "dark")),
    setTheme,
  };
}
