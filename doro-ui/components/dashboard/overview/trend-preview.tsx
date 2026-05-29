import { cn } from "@/lib/utils";

export type TrendPoint = {
  primary: number;
  secondary: number;
};

type TrendPreviewProps = {
  label: string;
  points?: TrendPoint[];
  seriesLabels?: [string, string];
  emptyText?: string;
};

export function TrendPreview({
  label,
  points = [],
  seriesLabels = ["上行", "下行"],
  emptyText = "暂无趋势数据，等待 Agent 指标采集",
}: TrendPreviewProps) {
  const visiblePoints = samplePoints(points, 48);
  const maxValue = Math.max(
    1,
    ...visiblePoints.flatMap((point) => [point.primary, point.secondary]),
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
      {visiblePoints.length > 0 ? (
        <div className="flex h-40 items-end gap-1.5 overflow-hidden">
          {visiblePoints.map((point, index) => (
            <div
              key={`${point.primary}-${point.secondary}-${index}`}
              className="flex h-full min-w-0 flex-1 items-end gap-0.5 rounded-md bg-muted px-0.5"
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
        <div className="flex h-40 items-center justify-center rounded-md border border-dashed bg-muted/40 px-4 text-center text-sm text-muted-foreground">
          {emptyText}
        </div>
      )}
    </div>
  );
}

function samplePoints(points: TrendPoint[], maxPoints: number) {
  if (points.length <= maxPoints) {
    return points;
  }

  const stride = Math.ceil(points.length / maxPoints);
  return points.filter((_, index) => index % stride === 0).slice(-maxPoints);
}
