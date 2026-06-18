"use client";

import * as React from "react";
import type { ComponentType } from "react";
import { Area, AreaChart, CartesianGrid, XAxis, YAxis } from "recharts";

import {
  ChartContainer,
  ChartLegend,
  ChartLegendContent,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from "@/components/ui/chart";
import { cn } from "@/lib/utils";

export type TrendPoint = {
  capturedAt: string;
  primary: number;
  secondary: number;
};

type TrendPreviewProps = {
  label: string;
  points?: TrendPoint[];
  seriesLabels?: [string, string];
  seriesIcons?: [SeriesIcon, SeriesIcon];
  emptyText?: string;
  valueFormatter?: (value: number) => string;
};

type SeriesIcon = ComponentType<{
  className?: string;
  "aria-hidden"?: boolean;
}>;

type ChartDatum = TrendPoint & {
  timeLabel: string;
};

const DEFAULT_VALUE_FORMATTER = new Intl.NumberFormat("zh-CN", {
  maximumFractionDigits: 1,
});

export function TrendPreview({
  label,
  points = [],
  seriesLabels = ["上行", "下行"],
  seriesIcons,
  emptyText = "暂无趋势数据，等待 Agent 指标采集",
  valueFormatter = (value) => DEFAULT_VALUE_FORMATTER.format(value),
}: TrendPreviewProps) {
  const gradientId = React.useId().replace(/:/g, "");
  const visiblePoints = samplePoints(points, 64);
  const chartData = visiblePoints.map<ChartDatum>((point) => ({
    ...point,
    timeLabel: formatShortTime(point.capturedAt),
  }));
  const chartConfig = {
    primary: {
      label: seriesLabels[0],
      icon: seriesIcons?.[0],
      color: "hsl(var(--chart-1))",
    },
    secondary: {
      label: seriesLabels[1],
      icon: seriesIcons?.[1],
      color: "hsl(var(--chart-2))",
    },
  } satisfies ChartConfig;

  return (
    <div className="rounded-lg border p-4">
      <div className="mb-4 flex items-center justify-between gap-3">
        <p className="text-sm font-medium">{label}</p>
      </div>
      {chartData.length > 0 ? (
        <ChartContainer config={chartConfig} className="h-[220px] w-full">
          <AreaChart
            accessibilityLayer
            data={chartData}
            margin={{ top: 8, right: 8, bottom: 0, left: 0 }}
          >
            <defs>
              <linearGradient
                id={`${gradientId}-primary`}
                x1="0"
                y1="0"
                x2="0"
                y2="1"
              >
                <stop
                  offset="5%"
                  stopColor="var(--color-primary)"
                  stopOpacity={0.42}
                />
                <stop
                  offset="95%"
                  stopColor="var(--color-primary)"
                  stopOpacity={0.05}
                />
              </linearGradient>
              <linearGradient
                id={`${gradientId}-secondary`}
                x1="0"
                y1="0"
                x2="0"
                y2="1"
              >
                <stop
                  offset="5%"
                  stopColor="var(--color-secondary)"
                  stopOpacity={0.34}
                />
                <stop
                  offset="95%"
                  stopColor="var(--color-secondary)"
                  stopOpacity={0.04}
                />
              </linearGradient>
            </defs>
            <CartesianGrid vertical={false} strokeDasharray="3 3" />
            <XAxis
              dataKey="capturedAt"
              tickLine={false}
              axisLine={false}
              tickMargin={10}
              minTickGap={26}
              tickFormatter={formatShortTime}
            />
            <YAxis
              width={64}
              tickLine={false}
              axisLine={false}
              tickMargin={8}
              tickCount={4}
              domain={[0, (dataMax: number) => Math.max(dataMax, 1)]}
              tickFormatter={(value) => valueFormatter(Number(value))}
            />
            <ChartTooltip
              cursor={false}
              content={
                <ChartTooltipContent
                  indicator="line"
                  labelFormatter={(value) => formatLongTime(String(value))}
                  formatter={(value, name, item) =>
                    tooltipValue(value, name, item.color, chartConfig, valueFormatter)
                  }
                />
              }
            />
            <ChartLegend content={<ChartLegendContent />} />
            <Area
              dataKey="secondary"
              type="monotone"
              fill={`url(#${gradientId}-secondary)`}
              stroke="var(--color-secondary)"
              strokeWidth={2}
              dot={false}
              activeDot={{ r: 4 }}
              name="secondary"
            />
            <Area
              dataKey="primary"
              type="monotone"
              fill={`url(#${gradientId}-primary)`}
              stroke="var(--color-primary)"
              strokeWidth={2}
              dot={false}
              activeDot={{ r: 4 }}
              name="primary"
            />
          </AreaChart>
        </ChartContainer>
      ) : (
        <div className="flex h-[220px] items-center justify-center rounded-md border border-dashed bg-muted/40 px-4 text-center text-sm text-muted-foreground">
          {emptyText}
        </div>
      )}
    </div>
  );
}

function tooltipValue(
  value: unknown,
  name: unknown,
  color: string | undefined,
  config: ChartConfig,
  valueFormatter: (value: number) => string,
) {
  const seriesKey = String(name);
  const numericValue = typeof value === "number" ? value : Number(value);
  const label = config[seriesKey]?.label ?? seriesKey;

  return (
    <>
      <span
        className="size-2.5 shrink-0 rounded-[2px]"
        style={{ backgroundColor: color }}
      />
      <span className="text-muted-foreground">{label}</span>
      <span className="ml-auto font-mono font-medium tabular-nums text-foreground">
        {Number.isFinite(numericValue) ? valueFormatter(numericValue) : "-"}
      </span>
    </>
  );
}

function formatShortTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

function formatLongTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

function samplePoints(points: TrendPoint[], maxPoints: number) {
  if (points.length <= maxPoints) {
    return points;
  }

  const stride = Math.ceil(points.length / maxPoints);
  return points.filter((_, index) => index % stride === 0).slice(-maxPoints);
}
