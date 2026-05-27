"use client";

import { useEffect, useState } from "react";
import { usePathname, useRouter } from "next/navigation";

import { AppShell } from "@/components/layout/app-shell";
import { currentUser } from "@/lib/control-plane-api";
import type { UserSummary } from "@/types/api";

type AuthState = "checking" | "ready";

type AuthGateProps = {
  children: React.ReactNode;
};

export function AuthGate({ children }: AuthGateProps) {
  const router = useRouter();
  const pathname = usePathname();
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
        正在验证登录状态...
      </div>
    );
  }

  if (!user) {
    return null;
  }

  return <AppShell user={user}>{children}</AppShell>;
}
