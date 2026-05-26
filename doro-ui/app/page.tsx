import { ArrowRight, Boxes, TerminalSquare } from "lucide-react";

import { Button } from "@/components/ui/button";

export default function Home() {
  return (
    <main className="min-h-screen bg-background text-foreground">
      <section className="mx-auto flex min-h-screen w-full max-w-6xl flex-col px-6 py-6">
        <header className="flex items-center justify-between border-b border-border pb-4">
          <div className="flex items-center gap-3">
            <div className="flex size-9 items-center justify-center rounded-md bg-primary text-primary-foreground">
              <Boxes className="size-4" aria-hidden="true" />
            </div>
            <span className="text-sm font-semibold tracking-wide">Doro</span>
          </div>
          <Button variant="ghost" size="sm">
            Console
          </Button>
        </header>

        <div className="grid flex-1 items-center gap-10 py-16 md:grid-cols-[1.08fr_0.92fr]">
          <div className="max-w-2xl">
            <p className="mb-4 inline-flex items-center gap-2 rounded-md border border-border px-3 py-1 text-sm text-muted-foreground">
              <TerminalSquare className="size-4" aria-hidden="true" />
              Frontend shell
            </p>
            <h1 className="text-4xl font-semibold leading-tight md:text-6xl">
              Doro UI
            </h1>
            <p className="mt-5 max-w-xl text-base leading-7 text-muted-foreground">
              A Next.js, Tailwind, and shadcn/ui workspace ready for product
              screens, shared components, and interface experiments.
            </p>
            <div className="mt-8 flex flex-wrap gap-3">
              <Button>
                Start building
                <ArrowRight className="size-4" aria-hidden="true" />
              </Button>
              <Button variant="outline">View components</Button>
            </div>
          </div>

          <div className="rounded-lg border border-border bg-card p-5 text-card-foreground shadow-sm">
            <div className="mb-5 flex items-center justify-between">
              <div>
                <h2 className="text-sm font-medium">Project stack</h2>
                <p className="text-sm text-muted-foreground">
                  Bun runtime, App Router, shadcn tokens
                </p>
              </div>
              <div className="size-2 rounded-full bg-primary" />
            </div>
            <div className="space-y-3">
              {["Next.js", "Tailwind CSS", "shadcn/ui"].map((item) => (
                <div
                  key={item}
                  className="flex items-center justify-between rounded-md border border-border px-3 py-2 text-sm"
                >
                  <span>{item}</span>
                  <span className="text-muted-foreground">ready</span>
                </div>
              ))}
            </div>
          </div>
        </div>
      </section>
    </main>
  );
}
