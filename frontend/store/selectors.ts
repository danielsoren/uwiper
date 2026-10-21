import type { ServiceItem, StartupItem, TaskItem } from '../lib/types';

/** Entries that point to files that are gone – the leftovers v0.1 can see. */
export function brokenEntries(
  startup: StartupItem[] | null,
  services: ServiceItem[] | null,
  tasks: TaskItem[] | null,
) {
  const s = (startup ?? []).filter((i) => i.fileExists === false);
  const v = (services ?? []).filter((i) => i.fileExists === false && !i.isWindows);
  const t = (tasks ?? []).filter((i) => i.fileExists === false && !i.isWindows);
  return { startup: s, services: v, tasks: t, total: s.length + v.length + t.length };
}
