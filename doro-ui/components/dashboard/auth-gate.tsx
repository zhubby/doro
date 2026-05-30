"use client";

import { useEffect, useState } from "react";
import { useTranslations } from "next-intl";

import { AppShell } from "@/components/layout/app-shell";
import { usePathname, useRouter } from "@/i18n/navigation";
import { currentUser } from "@/lib/control-plane-api";
import type { UserSummary } from "@/types/api";

type AuthState = "checking" | "ready";

type AuthGateProps = {
  children: React.ReactNode;
};

export function AuthGate({ children }: AuthGateProps) {
  const router = useRouter();
  const pathname = usePathname();
  const t = useTranslations("auth");
  const [state, setState] = useState<AuthState>("checking");
  const [user, setUser] = useState<UserSummary | null>(null);

  useEffect(() => {
    let cancelled = false;

    currentUser().then((result) => {
      if (cancelled) {
        return;
      }
      if (!result.data) {
        router.replace(`/login?next=${encodeURIComponent(pathname)}`);
        return;
      }
      setUser(result.data.user);
      setState("ready");
    });

    return () => {
      cancelled = true;
    };
  }, [pathname, router]);

  if (state === "checking") {
    return (
      <div className="flex min-h-screen items-center justify-center bg-background text-sm text-muted-foreground">
        {t("checking")}
      </div>
    );
  }

  if (!user) {
    return null;
  }

  return <AppShell user={user}>{children}</AppShell>;
}
