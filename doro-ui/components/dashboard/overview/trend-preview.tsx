import { trendBars } from "@/lib/mock-data";
import { cn } from "@/lib/utils";

export function TrendPreview({ label }: { label: string }) {
  return (
    <div className="rounded-lg border p-4">
      <div className="mb-4 flex items-center justify-between">
        <p className="text-sm font-medium">{label}</p>
        <div className="flex items-center gap-4 text-xs text-muted-foreground">
          <span>上行</span>
          <span>下行</span>
        </div>
      </div>
      <div className="flex h-40 items-end gap-2">
        {trendBars.map((height, index) => (
          <div
            key={`${height}-${index}`}
            className="flex flex-1 items-end rounded-md bg-muted"
          >
            <div
              className={cn(
                "w-full rounded-md bg-primary",
                index % 3 === 0 && "bg-primary/70",
              )}
              style={{ height: `${height}%` }}
            />
          </div>
        ))}
      </div>
    </div>
  );
}
