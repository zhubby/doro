import { PageSection } from "@/components/admin/page-section";
import { PageContainer } from "@/components/layout/page-container";

type PlaceholderPageProps = {
  title: string;
  description: string;
  items: string[];
};

export function PlaceholderPage({
  title: _title,
  description: _description,
  items,
}: PlaceholderPageProps) {
  return (
    <PageContainer>
      <PageSection>
        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
          {items.map((item) => (
            <div key={item} className="rounded-lg border p-4">
              <p className="text-sm font-medium">{item}</p>
              <p className="mt-2 text-xs text-muted-foreground">
                下一阶段接入真实数据与操作流程。
              </p>
            </div>
          ))}
        </div>
      </PageSection>
    </PageContainer>
  );
}
