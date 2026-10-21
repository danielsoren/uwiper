import { cn } from '../lib/cn';
import { Icon } from './Icon';

/** A square checkbox; `mixed` shows a dash for a partly selected group. */
export function Checkbox({
  checked,
  mixed,
  onChange,
  label,
  disabled,
}: {
  checked: boolean;
  mixed?: boolean;
  onChange: (next: boolean) => void;
  label: string;
  disabled?: boolean;
}) {
  const on = checked || mixed;
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={mixed ? 'mixed' : checked}
      aria-label={label}
      disabled={disabled}
      onClick={(e) => {
        e.stopPropagation();
        onChange(!checked);
      }}
      className={cn(
        'grid size-[18px] flex-none place-items-center rounded-[5px] border transition-colors disabled:opacity-35',
        on ? 'border-accent bg-accent text-white' : 'border-white/24 bg-white/4 enabled:hover:border-white/40',
      )}
    >
      {mixed ? <span className="h-0.5 w-2 rounded-full bg-white" /> : checked && <Icon name="check" size={12} stroke={3} />}
    </button>
  );
}
