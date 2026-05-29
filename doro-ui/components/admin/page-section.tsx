import { cn } from "@/lib/utils";

type PageSectionProps = {
  title?: string;
  description?: string;
  toolbar?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
  contentClassName?: string;
};

export function PageSection({
  title,
  description,
  toolbar,
  children,
  className,
  contentClassName,
}: PageSectionProps) {
  return (
    <section className={cn("flex flex-col", className)}>
      {(title || description || toolbar) && (
        <div className="mb-3 flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
          <div>
            {title ? (
              <h2 className="text-base font-semibold tracking-tight">{title}</h2>
            ) : null}
            {description ? (
              <p className="mt-1 text-sm text-muted-foreground">{description}</p>
            ) : null}
          </div>
          {toolbar ? <div className="flex flex-wrap gap-2">{toolbar}</div> : null}
        </div>
      )}
      <div className={cn("min-h-0", contentClassName)}>{children}</div>
    </section>
  );
}
