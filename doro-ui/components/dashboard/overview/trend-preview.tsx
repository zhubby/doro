import { cn } from "@/lib/utils";

export type TrendPoint = {
  primary: number;
  secondary: number;
};

type TrendPreviewProps = {
  label: string;
  points?: TrendPoint[];
  seriesLabels?: [string, string];
};

export function TrendPreview({
  label,
  points = [],
  seriesLabels = ["上行", "下行"],
}: TrendPreviewProps) {
  const maxValue = Math.max(
    1,
    ...points.flatMap((point) => [point.primary, point.secondary]),
  );

  return (
    <div className="rounded-lg border p-4">
      <div className="mb-4 flex items-center justify-between">
        <p className="text-sm font-medium">{label}</p>
        <div className="flex items-center gap-4 text-xs text-muted-foreground">
          <span>{seriesLabels[0]}</span>
          <span>{seriesLabels[1]}</span>
        </div>
      </div>
      {points.length > 0 ? (
        <div className="flex h-40 items-end gap-2">
          {points.map((point, index) => (
            <div
              key={`${point.primary}-${point.secondary}-${index}`}
              className="flex flex-1 items-end gap-1 rounded-md bg-muted px-1"
            >
              <div
                className={cn("w-full rounded-md bg-primary/70")}
                style={{ height: `${Math.max(2, (point.primary / maxValue) * 100)}%` }}
              />
              <div
                className={cn("w-full rounded-md bg-primary")}
                style={{ height: `${Math.max(2, (point.secondary / maxValue) * 100)}%` }}
              />
            </div>
          ))}
        </div>
      ) : (
        <div className="flex h-40 items-center justify-center rounded-md bg-muted text-sm text-muted-foreground">
          等待 Agent 上报趋势数据
        </div>
      )}
    </div>
  );
}
