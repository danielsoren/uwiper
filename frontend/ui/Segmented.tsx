import type { ReactNode } from 'react';
import { cn } from '../lib/cn';

interface Props<T extends string> {
  value: T;
  options: { value: T; label: ReactNode }[];
  onChange: (value: T) => void;
  label: string;
}

/** A row of mutually exclusive options, like NSSegmentedControl. */
export function Segmented<T extends string>({ value, options, onChange, label }: Props<T>) {
  return (
    <div role="group" aria-label={label} className="inline-flex flex-wrap rounded-control border border-line bg-fill-2 p-0.5">
      {options.map((o) => {
        const on = o.value === value;
        return (
          <button
            key={o.value}
            type="button"
            aria-pressed={on}
            onClick={() => onChange(o.value)}
            className={cn(
              'group h-7 rounded-[7px] px-3 text-meta font-medium',
              on ? 'bg-white/16 text-white shadow-[0_1px_2px_rgba(0,0,0,.35)]' : 'text-[#b4b4bb]',
            )}
          >
            {o.label}
          </button>
        );
      })}
    </div>
  );
}

/** A small count shown next to a segment's label. */
export function SegmentCount({ children }: { children: ReactNode }) {
  return <span className="ml-1 text-micro text-fg-4 group-aria-pressed:text-white/75">{children}</span>;
}
