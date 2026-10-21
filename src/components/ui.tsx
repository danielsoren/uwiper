import type { ReactNode } from 'react';
import { Icon, type IconName } from './Icon';

type ButtonKind = 'primary' | 'secondary' | 'danger' | 'ghost';

export function Button({
  kind = 'secondary',
  icon,
  children,
  onClick,
  disabled,
  small,
  title,
  ariaLabel,
}: {
  kind?: ButtonKind;
  icon?: IconName;
  children?: ReactNode;
  onClick?: () => void;
  disabled?: boolean;
  small?: boolean;
  title?: string;
  ariaLabel?: string;
}) {
  return (
    <button
      type="button"
      className={`btn btn-${kind}${small ? ' btn-small' : ''}`}
      onClick={onClick}
      disabled={disabled}
      title={title}
      aria-label={ariaLabel}
    >
      {icon && <Icon name={icon} size={small ? 14 : 16} stroke={1.9} />}
      {children}
    </button>
  );
}

export function Switch({
  checked,
  onChange,
  label,
  disabled,
  small,
}: {
  checked: boolean;
  onChange: (next: boolean) => void;
  label: string;
  disabled?: boolean;
  small?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      className={`switch${checked ? ' on' : ''}${small ? ' small' : ''}`}
      onClick={() => onChange(!checked)}
    >
      <span />
    </button>
  );
}

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  label,
}: {
  value: T;
  options: { value: T; label: ReactNode }[];
  onChange: (v: T) => void;
  label: string;
}) {
  return (
    <div className="segmented" role="group" aria-label={label}>
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          aria-pressed={o.value === value}
          className={o.value === value ? 'on' : ''}
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

export type PillTone = 'good' | 'warn' | 'bad' | 'dim' | 'accent';

export function Pill({ tone = 'dim', children }: { tone?: PillTone; children: ReactNode }) {
  return <span className={`pill pill-${tone}`}>{children}</span>;
}

export function Spinner({ size = 18 }: { size?: number }) {
  return <span className="spinner" style={{ width: size, height: size }} aria-hidden="true" />;
}

export function Card({ children, className = '' }: { children: ReactNode; className?: string }) {
  return <section className={`card ${className}`}>{children}</section>;
}

export function GroupTitle({ children, aside }: { children: ReactNode; aside?: ReactNode }) {
  return (
    <h2 className="group-title">
      <span>{children}</span>
      {aside && <span className="group-title-aside">{aside}</span>}
    </h2>
  );
}

export function Note({ icon = 'info', children, tone }: { icon?: IconName; children: ReactNode; tone?: 'warn' | 'good' }) {
  return (
    <div className={`note${tone ? ` note-${tone}` : ''}`}>
      <Icon name={icon} size={16} />
      <div>{children}</div>
    </div>
  );
}

export function Empty({
  icon,
  title,
  children,
  action,
}: {
  icon: IconName;
  title: string;
  children?: ReactNode;
  action?: ReactNode;
}) {
  return (
    <div className="empty">
      <span className="empty-icon">
        <Icon name={icon} size={30} />
      </span>
      <h2>{title}</h2>
      {children && <p>{children}</p>}
      {action}
    </div>
  );
}

export function Loading({ label }: { label: string }) {
  return (
    <div className="loading" role="status">
      <Spinner />
      <span>{label}</span>
    </div>
  );
}

export function ErrorBox({
  message,
  onRetry,
  retryLabel = 'Try again',
}: {
  message: string;
  onRetry?: () => void;
  retryLabel?: string;
}) {
  return (
    <div className="note note-warn" role="alert">
      <Icon name="warning" size={16} />
      <div className="note-row">
        <span>{message}</span>
        {onRetry && (
          <Button small onClick={onRetry}>
            {retryLabel}
          </Button>
        )}
      </div>
    </div>
  );
}

export function Mono({ children, className = '' }: { children: ReactNode; className?: string }) {
  return <span className={`mono ${className}`}>{children}</span>;
}
