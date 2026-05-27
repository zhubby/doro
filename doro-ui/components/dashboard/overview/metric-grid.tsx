import type { Metric } from "@/types/dashboard";

export function MetricGrid({ metrics }: { metrics: Metric[] }) {
  return (
    <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
      {metrics.map((metric) => (
        <div key={metric.label} className="rounded-lg border p-3">
          <p className="text-xs text-muted-foreground">{metric.label}</p>
          <p className="mt-1 text-sm font-semibold">{metric.value}</p>
        </div>
      ))}
    </div>
  );
}
