"use client";

import { Bell, RefreshCw, Send } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import { PageSection } from "@/components/admin/page-section";
import { Toolbar } from "@/components/admin/toolbar";
import { PageContainer } from "@/components/layout/page-container";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Select } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { useToastMessage } from "@/components/ui/use-toast-message";
import {
  getEmailNotificationSettings,
  getSystemNotificationSettings,
  testEmailNotification,
  updateEmailNotificationSettings,
  updateSystemNotificationSettings,
} from "@/lib/control-plane-api";
import type {
  EmailNotificationSettings,
  EmailSecurityMode,
  SystemNotificationSettings,
  UpdateEmailNotificationSettingsRequest,
} from "@/types/api";

type EmailFormState = {
  enabled: boolean;
  smtpHost: string;
  smtpPort: string;
  security: EmailSecurityMode;
  username: string;
  password: string;
  clearPassword: boolean;
  fromAddress: string;
  recipients: string;
  subjectPrefix: string;
  hasPassword: boolean;
};

type SystemNotificationFormState = {
  enabled: boolean;
};

const emptyForm: EmailFormState = {
  enabled: false,
  smtpHost: "",
  smtpPort: "587",
  security: "start_tls",
  username: "",
  password: "",
  clearPassword: false,
  fromAddress: "",
  recipients: "",
  subjectPrefix: "[Doro]",
  hasPassword: false,
};

const emptySystemForm: SystemNotificationFormState = {
  enabled: true,
};

const inputClass =
  "h-9 w-full rounded-md border bg-background px-3 text-sm outline-none ring-offset-background transition-colors placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring";

const textareaClass =
  "min-h-28 w-full rounded-md border bg-background px-3 py-2 text-sm outline-none ring-offset-background transition-colors placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring";

const securityOptions = [
  { value: "start_tls", label: "STARTTLS" },
  { value: "tls", label: "TLS" },
  { value: "none", label: "无加密" },
];

function settingsToForm(settings: EmailNotificationSettings): EmailFormState {
  return {
    enabled: settings.enabled,
    smtpHost: settings.smtp_host,
    smtpPort: String(settings.smtp_port),
    security: settings.security,
    username: settings.username ?? "",
    password: "",
    clearPassword: false,
    fromAddress: settings.from_address,
    recipients: settings.recipients.join("\n"),
    subjectPrefix: settings.subject_prefix,
    hasPassword: settings.has_password,
  };
}

function systemSettingsToForm(
  settings: SystemNotificationSettings,
): SystemNotificationFormState {
  return {
    enabled: settings.enabled,
  };
}

function recipientsFromText(value: string) {
  return value
    .split(/\r?\n|,/)
    .map((recipient) => recipient.trim())
    .filter(Boolean);
}

function formToRequest(form: EmailFormState): UpdateEmailNotificationSettingsRequest {
  return {
    enabled: form.enabled,
    smtp_host: form.smtpHost.trim(),
    smtp_port: Number.parseInt(form.smtpPort, 10) || 587,
    security: form.security,
    username: form.username.trim() || null,
    password: form.password.trim() || null,
    clear_password: form.clearPassword,
    from_address: form.fromAddress.trim(),
    recipients: recipientsFromText(form.recipients),
    subject_prefix: form.subjectPrefix.trim() || "[Doro]",
  };
}

export function NotificationsPage() {
  const [form, setForm] = useState<EmailFormState>(emptyForm);
  const [systemForm, setSystemForm] =
    useState<SystemNotificationFormState>(emptySystemForm);
  const [error, setError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [actionMessage, setActionMessage] = useState<string | null>(null);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [savePending, setSavePending] = useState(false);
  const [testPending, setTestPending] = useState(false);

  const recipientCount = useMemo(
    () => recipientsFromText(form.recipients).length,
    [form.recipients],
  );

  useToastMessage(error, {
    id: "notifications-api-error",
    kind: "error",
    prefix: "控制平面暂不可用：",
  });
  useToastMessage(actionError, {
    id: "notifications-action-error",
    kind: "error",
    prefix: "通知配置失败：",
  });
  useToastMessage(actionMessage, {
    id: "notifications-action-success",
    kind: "success",
  });

  async function load() {
    setIsRefreshing(true);
    const [emailResult, systemResult] = await Promise.all([
      getEmailNotificationSettings(),
      getSystemNotificationSettings(),
    ]);
    setIsRefreshing(false);
    if (!emailResult.data || !systemResult.data) {
      setError(
        emailResult.error ??
          systemResult.error ??
          "无法读取通知配置",
      );
      return;
    }
    setError(null);
    setForm(settingsToForm(emailResult.data.item));
    setSystemForm(systemSettingsToForm(systemResult.data.item));
  }

  useEffect(() => {
    load();
  }, []);

  async function handleSave() {
    setSavePending(true);
    setActionError(null);
    setActionMessage(null);
    const [emailResult, systemResult] = await Promise.all([
      updateEmailNotificationSettings(formToRequest(form)),
      updateSystemNotificationSettings(systemForm),
    ]);
    setSavePending(false);
    if (!emailResult.data || !systemResult.data) {
      setActionError(
        emailResult.error ??
          systemResult.error ??
          "保存失败",
      );
      return;
    }
    setForm(settingsToForm(emailResult.data.item));
    setSystemForm(systemSettingsToForm(systemResult.data.item));
    setActionMessage("通知配置已保存");
  }

  async function handleTest() {
    setTestPending(true);
    setActionError(null);
    setActionMessage(null);
    const result = await testEmailNotification();
    setTestPending(false);
    if (!result.data) {
      setActionError(result.error ?? "测试邮件发送失败");
      return;
    }
    setActionMessage("测试邮件已发送");
  }

  return (
    <PageContainer>
      <PageSection contentClassName="grid gap-3 md:grid-cols-3">
        <SummaryTile label="通知方式" value="站内 / 邮件" />
        <SummaryTile label="收件人" value={String(recipientCount)} />
        <SummaryTile
          label="配置状态"
          value={systemForm.enabled || form.enabled ? "启用" : "停用"}
          muted={!systemForm.enabled && !form.enabled}
        />
      </PageSection>

      <PageSection
        title="系统站内通知"
        description="告警触发和恢复时写入未读站内通知，首页会展示未读列表。"
        contentClassName="space-y-4"
      >
        <Toolbar
          right={
            <>
              <Button variant="outline" size="icon" aria-label="刷新" onClick={load}>
                <RefreshCw
                  className={isRefreshing ? "size-4 animate-spin" : "size-4"}
                  aria-hidden="true"
                />
              </Button>
              <Button onClick={handleSave} disabled={savePending || testPending}>
                {savePending ? "保存中" : "保存配置"}
              </Button>
            </>
          }
        />

        <div className="grid gap-4 rounded-lg border p-4 md:grid-cols-2">
          <Field label="启用站内通知">
            <div className="flex h-9 items-center gap-3 rounded-md border px-3">
              <Switch
                checked={systemForm.enabled}
                disabled={savePending}
                onCheckedChange={(enabled) => setSystemForm({ enabled })}
              />
              <span className="text-sm">{systemForm.enabled ? "启用" : "停用"}</span>
            </div>
          </Field>
          <div className="flex items-center gap-3 rounded-lg border bg-muted/30 px-3 py-2">
            <Bell className="size-4 text-muted-foreground" aria-hidden="true" />
            <div className="min-w-0">
              <p className="text-sm font-medium">首页未读通知</p>
              <p className="truncate text-xs text-muted-foreground">
                未读站内通知会显示在首页右侧卡片中。
              </p>
            </div>
          </div>
        </div>
      </PageSection>

      <PageSection
        title="邮件通知"
        description="告警触发和恢复时通过 SMTP 发送邮件，密码不会从控制面 API 回显。"
        contentClassName="space-y-4"
      >
        <Toolbar
          right={
            <>
              <Button variant="outline" size="icon" aria-label="刷新" onClick={load}>
                <RefreshCw
                  className={isRefreshing ? "size-4 animate-spin" : "size-4"}
                  aria-hidden="true"
                />
              </Button>
              <Button variant="outline" onClick={handleTest} disabled={testPending || savePending}>
                <Send className="size-4" aria-hidden="true" />
                {testPending ? "发送中" : "测试发送"}
              </Button>
              <Button onClick={handleSave} disabled={savePending || testPending}>
                {savePending ? "保存中" : "保存配置"}
              </Button>
            </>
          }
        />

        <div className="grid gap-4 rounded-lg border p-4 md:grid-cols-2">
          <Field label="启用邮件通知">
            <div className="flex h-9 items-center gap-3 rounded-md border px-3">
              <Switch
                checked={form.enabled}
                disabled={savePending}
                onCheckedChange={(enabled) => setForm({ ...form, enabled })}
              />
              <span className="text-sm">{form.enabled ? "启用" : "停用"}</span>
            </div>
          </Field>
          <Field label="安全模式">
            <Select
              value={form.security}
              disabled={savePending}
              onValueChange={(value) =>
                setForm({ ...form, security: value as EmailSecurityMode })
              }
              options={securityOptions}
            />
          </Field>
          <Field label="SMTP 主机">
            <input
              value={form.smtpHost}
              disabled={savePending}
              onChange={(event) => setForm({ ...form, smtpHost: event.target.value })}
              className={inputClass}
              placeholder="smtp.example.com"
            />
          </Field>
          <Field label="SMTP 端口">
            <input
              value={form.smtpPort}
              disabled={savePending}
              type="number"
              min="1"
              onChange={(event) => setForm({ ...form, smtpPort: event.target.value })}
              className={inputClass}
              placeholder="587"
            />
          </Field>
          <Field label="用户名">
            <input
              value={form.username}
              disabled={savePending}
              onChange={(event) => setForm({ ...form, username: event.target.value })}
              className={inputClass}
              placeholder="alerts@example.com"
            />
          </Field>
          <Field label="密码">
            <div className="space-y-2">
              <input
                value={form.password}
                disabled={savePending || form.clearPassword}
                type="password"
                onChange={(event) => setForm({ ...form, password: event.target.value })}
                className={inputClass}
                placeholder={form.hasPassword ? "已保存，留空则保持不变" : "SMTP 密码"}
              />
              <div className="flex items-center justify-between rounded-md border px-3 py-2">
                <span className="text-xs text-muted-foreground">
                  {form.hasPassword ? "当前已保存密码" : "当前未保存密码"}
                </span>
                <label className="flex items-center gap-2 text-xs">
                  <Switch
                    checked={form.clearPassword}
                    disabled={savePending || !form.hasPassword}
                    onCheckedChange={(clearPassword) =>
                      setForm({ ...form, clearPassword, password: "" })
                    }
                  />
                  清除
                </label>
              </div>
            </div>
          </Field>
          <Field label="发件人">
            <input
              value={form.fromAddress}
              disabled={savePending}
              onChange={(event) => setForm({ ...form, fromAddress: event.target.value })}
              className={inputClass}
              placeholder="Doro <alerts@example.com>"
            />
          </Field>
          <Field label="主题前缀">
            <input
              value={form.subjectPrefix}
              disabled={savePending}
              onChange={(event) => setForm({ ...form, subjectPrefix: event.target.value })}
              className={inputClass}
              placeholder="[Doro]"
            />
          </Field>
          <Field label="收件人" className="md:col-span-2">
            <textarea
              value={form.recipients}
              disabled={savePending}
              onChange={(event) => setForm({ ...form, recipients: event.target.value })}
              className={textareaClass}
              placeholder="ops@example.com&#10;admin@example.com"
            />
          </Field>
        </div>
      </PageSection>
    </PageContainer>
  );
}

function SummaryTile({
  label,
  value,
  muted = false,
}: {
  label: string;
  value: string;
  muted?: boolean;
}) {
  return (
    <div className="rounded-lg border p-4">
      <div className="flex items-center justify-between gap-2 text-sm text-muted-foreground">
        <span className="flex items-center gap-2">
          <Bell className="size-4" aria-hidden="true" />
          {label}
        </span>
        {muted ? <Badge variant="outline">未启用</Badge> : null}
      </div>
      <p className="mt-2 text-2xl font-semibold tracking-tight">{value}</p>
    </div>
  );
}

function Field({
  label,
  children,
  className,
}: {
  label: string;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <label className={className}>
      <span className="mb-2 block text-sm font-medium">{label}</span>
      {children}
    </label>
  );
}
