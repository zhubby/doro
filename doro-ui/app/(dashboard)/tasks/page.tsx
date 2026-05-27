import { TasksPage } from "@/components/dashboard/tasks/tasks-page";
import { getTasks } from "@/lib/control-plane-api";

export const dynamic = "force-dynamic";

export default async function TasksRoute() {
  const tasks = await getTasks();

  return <TasksPage tasks={tasks.data?.items ?? []} apiError={tasks.error} />;
}
