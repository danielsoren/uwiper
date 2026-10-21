import { cn } from '../lib/cn';

interface Props {
  checked: boolean;
  onChange: (next: boolean) => void;
  label: string;
  disabled?: boolean;
  small?: boolean;
}

/** macOS-style toggle. */
export function Switch({ checked, onChange, label, disabled, small }: Props) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cn(
        'flex-none rounded-full p-0.5 transition-colors duration-200 disabled:opacity-40',
        small ? 'h-5 w-[34px]' : 'h-6 w-10',
        checked ? 'bg-accent' : 'bg-white/18',
      )}
    >
      <span
        className={cn(
          'block rounded-full bg-white shadow-[0_1px_3px_rgba(0,0,0,.4)] transition-transform duration-200',
          small ? 'size-4' : 'size-5',
          checked && (small ? 'translate-x-3.5' : 'translate-x-4'),
        )}
      />
    </button>
  );
}
