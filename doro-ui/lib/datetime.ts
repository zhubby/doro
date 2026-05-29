const SECOND = 1000;
const MINUTE = 60 * SECOND;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;
const WEEK = 7 * DAY;

type RelativeTimeOptions = {
  emptyText?: string;
  now?: Date;
};

export function formatRelativeTime(
  value: string | Date | null | undefined,
  options: RelativeTimeOptions = {},
) {
  if (!value) {
    return options.emptyText ?? "-";
  }

  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) {
    return String(value);
  }

  const now = options.now ?? new Date();
  const diff = now.getTime() - date.getTime();
  const absDiff = Math.abs(diff);
  const suffix = diff >= 0 ? "前" : "后";

  if (absDiff < 10 * SECOND) {
    return "刚刚";
  }

  if (absDiff < MINUTE) {
    return `${Math.floor(absDiff / SECOND)} 秒${suffix}`;
  }

  if (absDiff < HOUR) {
    return `${Math.floor(absDiff / MINUTE)} 分钟${suffix}`;
  }

  if (absDiff < DAY) {
    return `${Math.floor(absDiff / HOUR)} 小时${suffix}`;
  }

  if (absDiff < WEEK) {
    return `${Math.floor(absDiff / DAY)} 天${suffix}`;
  }

  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}
