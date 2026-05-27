"use client";

import { FormEvent, useEffect, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { LogIn, UserPlus } from "lucide-react";

import { authStatus, login, register } from "@/lib/control-plane-api";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

export function LoginPage() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const [registrationOpen, setRegistrationOpen] = useState(false);
  const [username, setUsername] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  useEffect(() => {
    authStatus().then((result) => {
      setRegistrationOpen(Boolean(result.data?.registration_open));
      if (result.error) {
        setError(result.error);
      }
    });
  }, []);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setPending(true);
    setError(null);

    const result = registrationOpen
      ? await register({
          username,
          display_name: displayName,
          password,
        })
      : await login({
          username,
          password,
        });

    setPending(false);
    if (!result.data) {
      setError(result.error ?? "认证失败");
      return;
    }

    router.replace(searchParams.get("next") ?? "/");
  }

  return (
    <main className="flex min-h-screen items-center justify-center bg-background px-4 py-10 text-foreground">
      <Card className="w-full max-w-md">
        <CardHeader>
          <div className="mb-2 flex size-10 items-center justify-center rounded-md bg-primary text-primary-foreground">
            {registrationOpen ? (
              <UserPlus className="size-5" aria-hidden="true" />
            ) : (
              <LogIn className="size-5" aria-hidden="true" />
            )}
          </div>
          <CardTitle>{registrationOpen ? "创建管理员" : "登录 Doro"}</CardTitle>
          <CardDescription>
            {registrationOpen
              ? "初始化首个管理员账号，之后将关闭公开注册。"
              : "使用管理员账号进入控制平面。"}
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form className="space-y-4" onSubmit={submit}>
            <label className="block space-y-2 text-sm">
              <span className="font-medium">用户名</span>
              <input
                className="h-10 w-full rounded-md border bg-background px-3 outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
                value={username}
                onChange={(event) => setUsername(event.target.value)}
                autoComplete="username"
                minLength={3}
                maxLength={64}
                required
              />
            </label>

            {registrationOpen ? (
              <label className="block space-y-2 text-sm">
                <span className="font-medium">显示名称</span>
                <input
                  className="h-10 w-full rounded-md border bg-background px-3 outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
                  value={displayName}
                  onChange={(event) => setDisplayName(event.target.value)}
                  autoComplete="name"
                />
              </label>
            ) : null}

            <label className="block space-y-2 text-sm">
              <span className="font-medium">密码</span>
              <input
                className="h-10 w-full rounded-md border bg-background px-3 outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
                type="password"
                value={password}
                onChange={(event) => setPassword(event.target.value)}
                autoComplete={registrationOpen ? "new-password" : "current-password"}
                minLength={10}
                required
              />
            </label>

            {error ? (
              <div className="rounded-md border border-destructive/30 p-3 text-sm text-destructive">
                {error}
              </div>
            ) : null}

            <Button className="w-full" type="submit" disabled={pending}>
              {pending ? "处理中..." : registrationOpen ? "创建并进入" : "登录"}
            </Button>
          </form>
        </CardContent>
      </Card>
    </main>
  );
}
