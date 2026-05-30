const SECOND = 1000;
const MINUTE = 60 * SECOND;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;
const WEEK = 7 * DAY;

type RelativeTimeOptions = {
  emptyText?: string;
  locale?: string;
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
  const locale = options.locale ?? "zh-CN";

  if (absDiff < 10 * SECOND) {
    return locale === "zh-CN" ? "刚刚" : "Just now";
  }

  if (absDiff < MINUTE) {
    return new Intl.RelativeTimeFormat(locale, { numeric: "auto" }).format(
      diff >= 0 ? -Math.floor(absDiff / SECOND) : Math.floor(absDiff / SECOND),
      "second",
    );
  }

  if (absDiff < HOUR) {
    return new Intl.RelativeTimeFormat(locale, { numeric: "auto" }).format(
      diff >= 0 ? -Math.floor(absDiff / MINUTE) : Math.floor(absDiff / MINUTE),
      "minute",
    );
  }

  if (absDiff < DAY) {
    return new Intl.RelativeTimeFormat(locale, { numeric: "auto" }).format(
      diff >= 0 ? -Math.floor(absDiff / HOUR) : Math.floor(absDiff / HOUR),
      "hour",
    );
  }

  if (absDiff < WEEK) {
    return new Intl.RelativeTimeFormat(locale, { numeric: "auto" }).format(
      diff >= 0 ? -Math.floor(absDiff / DAY) : Math.floor(absDiff / DAY),
      "day",
    );
  }

  return new Intl.DateTimeFormat(locale, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}
