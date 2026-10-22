import { cn } from '../../lib/cn';
import { formatBytes, formatDate } from '../../lib/format';
import type { DriverItem } from '../../lib/types';
import { Checkbox, IconTile, ListCard, Pill, type PillTone } from '../../ui';

const STATE: Record<'inUse' | 'unknown' | 'old' | 'spare', { label: string; tone: PillTone; hint: string }> = {
  inUse: { label: 'In use', tone: 'good', hint: 'A device, a service or a printer uses this package' },
  unknown: { label: 'Unknown', tone: 'dim', hint: "UWiper can't tell whether something uses it, so it's never removed" },
  old: { label: 'Older version', tone: 'warn', hint: 'A newer version of this package is in the Driver Store' },
  spare: { label: 'Not in use', tone: 'dim', hint: 'No device is set up with it, but it is the only version – it may be for a device you connect now and then' },
};

/** "Realtek Semiconductor Corp." reads as "Realtek Semiconductor". */
const shortProvider = (p: string) => p.replace(/,?\s+(inc\.?|corp\.?|corporation|co\.,? ltd\.?|ltd\.?|llc|gmbh|ag)$/i, '');

/** The provider, unless the device name already starts with it. */
function providerOf(d: DriverItem) {
  const provider = shortProvider(d.provider);
  const brand = provider.split(/[\s(]/)[0].toLowerCase();
  return d.device?.toLowerCase().startsWith(brand) ? null : provider;
}

/** Driver packages, each version of a driver next to the others, with a checkbox on the ones that can go. */
export function DriverList({
  items,
  selected,
  onChange,
  canRemove,
}: {
  items: DriverItem[];
  selected: Set<string>;
  onChange: (next: Set<string>) => void;
  canRemove: (d: DriverItem) => boolean;
}) {
  const toggle = (id: string, on: boolean) => {
    const next = new Set(selected);
    if (on) next.add(id);
    else next.delete(id);
    onChange(next);
  };

  return (
    <ListCard>
      {items.map((d) => {
        const allowed = canRemove(d);
        const on = selected.has(d.id);
        const state = STATE[d.usage === 'used' ? 'inUse' : d.usage === 'unknown' ? 'unknown' : d.old ? 'old' : 'spare'];
        const name = d.device ?? shortProvider(d.provider);
        return (
          <div
            key={d.id}
            onClick={() => allowed && toggle(d.id, !on)}
            className={cn('flex min-h-[60px] items-center gap-3.5 px-4 py-2.5', allowed && 'cursor-pointer hover:bg-hover')}
          >
            <Checkbox label={`Remove ${name} ${d.version ?? ''}`} checked={on} disabled={!allowed} onChange={(v) => toggle(d.id, v)} />
            <IconTile icon="cpu" tone={d.old && d.usage === 'unused' ? 'orange' : 'accent'} small />
            <span className="flex min-w-0 flex-[1_1_200px] flex-col gap-0.5">
              <span className="truncate text-label font-semibold">{name}</span>
              <span className="truncate text-small text-fg-3">
                {[d.device && providerOf(d), d.class, d.version, d.date && formatDate(d.date)].filter(Boolean).join(' · ')}
              </span>
            </span>
            <span title={`Published as ${d.id}`} className="w-[150px] flex-none truncate font-mono text-[11.5px] text-fg-3 max-[1100px]:hidden">
              {d.inf ?? d.id}
            </span>
            <span title={state.hint} className="flex w-[104px] flex-none justify-end">
              <Pill tone={state.tone}>{state.label}</Pill>
            </span>
            <span className="w-16 flex-none text-right text-meta tabular-nums text-fg-3">{d.sizeBytes ? formatBytes(d.sizeBytes) : ''}</span>
          </div>
        );
      })}
    </ListCard>
  );
}
