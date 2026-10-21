import type { ReactNode } from 'react';
import { cn } from '../lib/cn';
import { Icon, type IconName } from './Icon';
import { Spinner } from './Spinner';

export type ButtonKind = 'primary' | 'secondary' | 'danger' | 'ghost';

const KIND: Record<ButtonKind, string> = {
  primary:
    'bg-accent font-semibold text-white shadow-[inset_0_1px_0_rgba(255,255,255,.18),0_1px_2px_rgba(0,0,0,.3)] enabled:hover:brightness-115',
  secondary: 'border-white/10 bg-fill-2 enabled:hover:brightness-115',
  danger: 'bg-danger font-semibold text-white enabled:hover:brightness-115',
  ghost: 'bg-transparent text-fg-2 enabled:hover:bg-hover',
};

interface Props {
  kind?: ButtonKind;
  icon?: IconName;
  children?: ReactNode;
  onClick?: () => void;
  disabled?: boolean;
  /** Shows a spinner in place of the icon and blocks clicks. */
  busy?: boolean;
  small?: boolean;
  title?: string;
  ariaLabel?: string;
  className?: string;
}

export function Button({ kind = 'secondary', icon, children, onClick, disabled, busy, small, title, ariaLabel, className }: Props) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled || busy}
      aria-busy={busy || undefined}
      title={title}
      aria-label={ariaLabel}
      className={cn(
        'inline-flex items-center justify-center gap-2 whitespace-nowrap border border-transparent font-medium',
        busy ? 'cursor-progress' : 'disabled:opacity-45',
        small ? 'h-[30px] rounded-lg px-[11px] text-meta' : 'h-9 rounded-control px-3.5 text-body',
        KIND[kind],
        className,
      )}
    >
      {busy ? <Spinner size={small ? 13 : 15} /> : icon && <Icon name={icon} size={small ? 14 : 16} stroke={1.9} />}
      {children}
    </button>
  );
}
