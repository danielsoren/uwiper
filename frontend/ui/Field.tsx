import { cn } from '../lib/cn';
import { Icon, type IconName } from './Icon';

interface Props {
  value: string;
  onChange: (value: string) => void;
  label: string;
  icon: IconName;
  placeholder?: string;
  mono?: boolean;
  onEnter?: () => void;
  onEscape?: () => void;
  className?: string;
}

/** A text input with a leading icon: search boxes, paths, registry keys. */
export function Field({ value, onChange, label, icon, placeholder, mono, onEnter, onEscape, className }: Props) {
  return (
    <label
      className={cn(
        'flex min-w-0 items-center gap-2 rounded-control border px-3 text-fg-4 focus-within:shadow-[0_0_0_2px_var(--color-focus)]',
        mono ? 'h-8 rounded-lg border-white/14 bg-black/25 px-2.5' : 'h-9 border-line-2 bg-fill-2',
        className,
      )}
    >
      <Icon name={icon} size={15} stroke={mono ? 1.8 : 2} />
      <input
        type={mono ? 'text' : 'search'}
        spellCheck={false}
        aria-label={label}
        placeholder={placeholder}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Enter') onEnter?.();
          if (e.key === 'Escape') onEscape?.();
        }}
        className={cn('min-w-0 flex-1 bg-transparent text-fg outline-none', mono && 'font-mono text-small')}
      />
    </label>
  );
}
