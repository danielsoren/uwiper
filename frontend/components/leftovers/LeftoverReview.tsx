import { cn } from '../../lib/cn';
import { ago, formatBytes } from '../../lib/format';
import type { Confidence, Leftover } from '../../lib/types';
import { Checkbox, Icon, Pill, type IconName, type PillTone } from '../../ui';

const GROUPS: { title: string; kinds: Leftover['kind'][] }[] = [
  { title: 'Apps that are gone', kinds: ['app'] },
  { title: 'Services, tasks & startup', kinds: ['service', 'task', 'startup'] },
  { title: 'Files & folders', kinds: ['folder', 'file', 'shortcut'] },
  { title: 'Registry', kinds: ['registry'] },
  { title: 'PATH entries', kinds: ['environment'] },
];

const KIND_ICON: Record<Leftover['kind'], IconName> = {
  app: 'apps',
  environment: 'code',
  folder: 'folder',
  file: 'file',
  shortcut: 'link',
  registry: 'cube',
  service: 'cog',
  task: 'calendar',
  startup: 'power',
};

const CONFIDENCE: Record<Confidence, { tone: PillTone; label: string; hint: string }> = {
  high: { tone: 'good', label: 'High', hint: "Certainly the app's" },
  medium: { tone: 'warn', label: 'Medium', hint: 'Named after the app' },
  low: { tone: 'dim', label: 'Low', hint: 'Possibly shared – check before removing' },
};

/** What gets checked for you after a scan, per the Safety setting. */
export function preselect(items: Leftover[], mode: 'high' | 'high-medium' | 'all', canRemove: (l: Leftover) => boolean) {
  const levels: Confidence[] = mode === 'high' ? ['high'] : mode === 'high-medium' ? ['high', 'medium'] : ['high', 'medium', 'low'];
  return new Set(items.filter((l) => levels.includes(l.confidence) && canRemove(l)).map((l) => l.id));
}

/** Leftovers a scan found, grouped by kind, each with a checkbox. */
export function LeftoverReview({
  items,
  selected,
  onChange,
  canRemove,
}: {
  items: Leftover[];
  selected: Set<string>;
  onChange: (next: Set<string>) => void;
  /** False for items that need administrator rights UWiper doesn't have. */
  canRemove: (l: Leftover) => boolean;
}) {
  const toggle = (ids: string[], on: boolean) => {
    const next = new Set(selected);
    for (const id of ids) {
      if (on) next.add(id);
      else next.delete(id);
    }
    onChange(next);
  };

  return (
    <div className="flex flex-col gap-4">
      {GROUPS.map((group) => {
        const rows = items.filter((l) => group.kinds.includes(l.kind));
        if (rows.length === 0) return null;
        const ids = rows.filter(canRemove).map((l) => l.id);
        const picked = ids.filter((id) => selected.has(id)).length;
        const bytes = rows.reduce((s, l) => s + (l.sizeBytes ?? 0), 0);
        return (
          <section key={group.title} className="overflow-hidden rounded-card border border-line-2 bg-fill">
            <div className="flex min-h-11 items-center gap-3 border-b border-line px-4 py-2">
              <Checkbox
                label={`Select all: ${group.title}`}
                checked={ids.length > 0 && picked === ids.length}
                mixed={picked > 0 && picked < ids.length}
                disabled={ids.length === 0}
                onChange={(on) => toggle(ids, on)}
              />
              <h3 className="flex-1 text-label font-semibold">{group.title}</h3>
              <span className="text-small text-fg-3">
                {rows.length}
                {bytes > 0 && ` · ${formatBytes(bytes)}`}
              </span>
            </div>
            {rows.map((l) => {
              const allowed = canRemove(l);
              const on = selected.has(l.id);
              return (
                <div
                  key={l.id}
                  onClick={() => allowed && toggle([l.id], !on)}
                  className={cn(
                    'flex items-center gap-3 border-line px-4 py-2.5 [&+&]:border-t',
                    allowed ? 'cursor-pointer hover:bg-hover' : 'opacity-55',
                  )}
                >
                  <Checkbox label={`Remove ${l.label}`} checked={on} disabled={!allowed} onChange={(v) => toggle([l.id], v)} />
                  <Icon name={KIND_ICON[l.kind]} size={16} className="flex-none text-fg-3" />
                  <div className="min-w-0 flex-1">
                    <div title={l.label} className="truncate font-mono text-small text-fg-path">
                      {l.label}
                    </div>
                    <div className="mt-0.5 truncate text-small text-fg-3">
                      {l.reason}
                      {l.modified && ` · changed ${ago(l.modified)}`}
                    </div>
                  </div>
                  {!allowed && <Pill tone="warn">Needs admin</Pill>}
                  <span title={CONFIDENCE[l.confidence].hint}>
                    <Pill tone={CONFIDENCE[l.confidence].tone}>{CONFIDENCE[l.confidence].label}</Pill>
                  </span>
                  <span className="w-16 flex-none text-right text-meta tabular-nums text-fg-3">
                    {l.sizeBytes ? formatBytes(l.sizeBytes) : ''}
                  </span>
                </div>
              );
            })}
          </section>
        );
      })}
    </div>
  );
}
