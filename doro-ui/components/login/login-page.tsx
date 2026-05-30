"use client";

import { FormEvent, useEffect, useState } from "react";
import { LogIn, UserPlus } from "lucide-react";
import { useTranslations } from "next-intl";

import { authStatus, login, register } from "@/lib/control-plane-api";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { useRouter } from "@/i18n/navigation";
import { useSearchParams } from "next/navigation";

export function LoginPage() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const t = useTranslations("auth");
  const tCommon = useTranslations("common");
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
      setError(result.error ?? tCommon("errors.authFailed"));
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
          <CardTitle>
            {registrationOpen ? t("registerTitle") : t("loginTitle")}
          </CardTitle>
          <CardDescription>
            {registrationOpen
              ? t("registerDescription")
              : t("loginDescription")}
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form className="space-y-4" onSubmit={submit}>
            <label className="block space-y-2 text-sm">
              <span className="font-medium">{t("username")}</span>
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
                <span className="font-medium">{t("displayName")}</span>
                <input
                  className="h-10 w-full rounded-md border bg-background px-3 outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
                  value={displayName}
                  onChange={(event) => setDisplayName(event.target.value)}
                  autoComplete="name"
                />
              </label>
            ) : null}

            <label className="block space-y-2 text-sm">
              <span className="font-medium">{t("password")}</span>
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
              {pending
                ? t("pending")
                : registrationOpen
                  ? t("registerSubmit")
                  : t("loginSubmit")}
            </Button>
          </form>
        </CardContent>
      </Card>
    </main>
  );
}
