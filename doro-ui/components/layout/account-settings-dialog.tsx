"use client";

import {
  Check,
  KeyRound,
  Languages,
  Loader2,
  Settings,
  UserRound,
} from "lucide-react";
import { useEffect, useState } from "react";
import { useLocale, useTranslations } from "next-intl";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { usePathname, useRouter } from "@/i18n/navigation";
import { locales, type AppLocale } from "@/i18n/routing";
import {
  changeCurrentUserPassword,
  clearAuth,
  updateCurrentUser,
} from "@/lib/control-plane-api";
import { cn } from "@/lib/utils";
import type { UserSummary } from "@/types/api";

type AccountSettingsDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  initialTab: AccountDialogTab;
  user: UserSummary;
  onUserChange: (user: UserSummary) => void;
};

export type AccountDialogTab = "profile" | "password" | "preferences";

export function AccountSettingsDialog({
  open,
  onOpenChange,
  initialTab,
  user,
  onUserChange,
}: AccountSettingsDialogProps) {
  const t = useTranslations("account");
  const tCommon = useTranslations("common");
  const router = useRouter();
  const pathname = usePathname();
  const locale = useLocale() as AppLocale;
  const [tab, setTab] = useState<AccountDialogTab>(initialTab);
  const [displayName, setDisplayName] = useState(user.display_name);
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [profileStatus, setProfileStatus] = useState<FormStatus>("idle");
  const [passwordStatus, setPasswordStatus] = useState<FormStatus>("idle");

  const profilePending = profileStatus === "pending";
  const passwordPending = passwordStatus === "pending";
  const displayNameValue = user.display_name || user.username;
  const initials = displayNameValue.trim().slice(0, 1).toUpperCase();

  useEffect(() => {
    if (open) {
      setTab(initialTab);
      setDisplayName(user.display_name);
      setProfileStatus("idle");
      setPasswordStatus("idle");
      setCurrentPassword("");
      setNewPassword("");
      setConfirmPassword("");
    }
  }, [initialTab, open, user.display_name]);

  async function submitProfile(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setProfileStatus("pending");

    const result = await updateCurrentUser({ display_name: displayName });
    if (!result.data) {
      setProfileStatus("idle");
      toast.error(result.error ?? t("errors.profileFailed"));
      return;
    }

    onUserChange(result.data.user);
    setDisplayName(result.data.user.display_name);
    setProfileStatus("idle");
    toast.success(t("profileSaved"));
  }

  async function submitPassword(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (newPassword !== confirmPassword) {
      setPasswordStatus("idle");
      toast.error(t("errors.passwordMismatch"));
      return;
    }

    setPasswordStatus("pending");
    const result = await changeCurrentUserPassword({
      current_password: currentPassword,
      new_password: newPassword,
    });
    if (result.error) {
      setPasswordStatus("idle");
      toast.error(result.error ?? t("errors.passwordFailed"));
      return;
    }

    clearAuth();
    onOpenChange(false);
    router.replace("/login");
  }

  function switchLocale(targetLocale: AppLocale) {
    if (targetLocale === locale) {
      return;
    }
    router.replace(pathname, { locale: targetLocale });
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[min(760px,calc(100dvh-2rem))] overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>{t("title")}</DialogTitle>
          <DialogDescription>{t("description")}</DialogDescription>
        </DialogHeader>

        <div className="flex items-center gap-3 rounded-lg border bg-muted/30 p-3">
          <span className="flex size-11 shrink-0 items-center justify-center rounded-full bg-primary text-sm font-semibold text-primary-foreground">
            {initials}
          </span>
          <div className="min-w-0">
            <p className="truncate text-sm font-semibold">{displayNameValue}</p>
            <p className="truncate text-xs text-muted-foreground">
              @{user.username} · {user.role}
            </p>
          </div>
        </div>

        <Tabs
          value={tab}
          onValueChange={(value) => setTab(value as AccountDialogTab)}
          className="space-y-4"
        >
          <TabsList className="grid h-auto w-full grid-cols-3">
            <TabsTrigger className="gap-2" value="profile">
              <UserRound className="size-4" aria-hidden="true" />
              {t("tabs.profile")}
            </TabsTrigger>
            <TabsTrigger className="gap-2" value="password">
              <KeyRound className="size-4" aria-hidden="true" />
              {t("tabs.password")}
            </TabsTrigger>
            <TabsTrigger className="gap-2" value="preferences">
              <Settings className="size-4" aria-hidden="true" />
              {t("tabs.preferences")}
            </TabsTrigger>
          </TabsList>

          <TabsContent value="profile">
            <form className="space-y-4" onSubmit={submitProfile}>
              <div className="grid gap-3 sm:grid-cols-2">
                <ReadOnlyField label={t("fields.username")} value={user.username} />
                <ReadOnlyField label={t("fields.role")} value={user.role} />
              </div>
              <label className="block space-y-2 text-sm">
                <span className="font-medium">{t("fields.displayName")}</span>
                <input
                  className="h-10 w-full rounded-md border bg-background px-3 outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
                  value={displayName}
                  onChange={(event) => setDisplayName(event.target.value)}
                  autoComplete="name"
                  disabled={profilePending}
                />
              </label>
              <div className="flex justify-end gap-2">
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => onOpenChange(false)}
                >
                  {tCommon("actions.cancel")}
                </Button>
                <Button type="submit" disabled={profilePending}>
                  {profilePending ? (
                    <Loader2 className="size-4 animate-spin" aria-hidden="true" />
                  ) : null}
                  {tCommon("actions.save")}
                </Button>
              </div>
            </form>
          </TabsContent>

          <TabsContent value="password">
            <form className="space-y-4" onSubmit={submitPassword}>
              <PasswordField
                label={t("fields.currentPassword")}
                value={currentPassword}
                onChange={setCurrentPassword}
                autoComplete="current-password"
                disabled={passwordPending}
              />
              <PasswordField
                label={t("fields.newPassword")}
                value={newPassword}
                onChange={setNewPassword}
                autoComplete="new-password"
                disabled={passwordPending}
              />
              <PasswordField
                label={t("fields.confirmPassword")}
                value={confirmPassword}
                onChange={setConfirmPassword}
                autoComplete="new-password"
                disabled={passwordPending}
              />
              <p className="text-xs text-muted-foreground">
                {t("passwordHelper")}
              </p>
              <div className="flex justify-end gap-2">
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => onOpenChange(false)}
                >
                  {tCommon("actions.cancel")}
                </Button>
                <Button type="submit" disabled={passwordPending}>
                  {passwordPending ? (
                    <Loader2 className="size-4 animate-spin" aria-hidden="true" />
                  ) : null}
                  {t("changePassword")}
                </Button>
              </div>
            </form>
          </TabsContent>

          <TabsContent value="preferences">
            <div className="space-y-3">
              <div>
                <h3 className="text-sm font-semibold">{t("language.title")}</h3>
                <p className="text-sm text-muted-foreground">
                  {t("language.description")}
                </p>
              </div>
              <div className="grid gap-2 sm:grid-cols-2">
                {locales.map((targetLocale) => {
                  const active = targetLocale === locale;
                  return (
                    <button
                      key={targetLocale}
                      type="button"
                      className={cn(
                        "flex items-center justify-between rounded-md border bg-background px-3 py-3 text-left text-sm transition-colors hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                        active && "border-primary bg-accent text-accent-foreground",
                      )}
                      onClick={() => switchLocale(targetLocale)}
                    >
                      <span className="flex items-center gap-2">
                        <Languages className="size-4" aria-hidden="true" />
                        {tCommon(`language.${targetLocale}`)}
                      </span>
                      {active ? (
                        <Check className="size-4 text-primary" aria-hidden="true" />
                      ) : null}
                    </button>
                  );
                })}
              </div>
            </div>
          </TabsContent>
        </Tabs>
      </DialogContent>
    </Dialog>
  );
}

type FormStatus = "idle" | "pending";

function ReadOnlyField({ label, value }: { label: string; value: string }) {
  return (
    <label className="block space-y-2 text-sm">
      <span className="font-medium">{label}</span>
      <input
        className="h-10 w-full rounded-md border bg-muted px-3 text-muted-foreground outline-none"
        value={value}
        readOnly
      />
    </label>
  );
}

function PasswordField({
  label,
  value,
  onChange,
  autoComplete,
  disabled,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  autoComplete: string;
  disabled: boolean;
}) {
  return (
    <label className="block space-y-2 text-sm">
      <span className="font-medium">{label}</span>
      <input
        className="h-10 w-full rounded-md border bg-background px-3 outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
        type="password"
        value={value}
        onChange={(event) => onChange(event.target.value)}
        autoComplete={autoComplete}
        minLength={10}
        required
        disabled={disabled}
      />
    </label>
  );
}
