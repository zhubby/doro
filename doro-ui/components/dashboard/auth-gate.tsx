"use client";

import { useEffect, useState } from "react";
import { usePathname, useRouter } from "next/navigation";

import { currentUser } from "@/lib/control-plane-api";

type AuthState = "checking" | "ready";

export function AuthGate({ children }: { children: React.ReactNode }) {
  const router = useRouter();
  const pathname = usePathname();
  const [state, setState] = useState<AuthState>("checking");

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

  return children;
}
