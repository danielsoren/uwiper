import type { ReactNode } from 'react';
import { Button } from './Button';
import { Icon, type IconName } from './Icon';

/** A quiet line of explanation with an icon. */
export function Note({ icon = 'info', children }: { icon?: IconName; children: ReactNode }) {
  return (
    <div className="flex items-start gap-2.5 px-1 text-small leading-normal text-fg-3 [&_strong]:font-semibold [&_strong]:text-fg-2">
      <Icon name={icon} size={16} className="mt-px" />
      <div>{children}</div>
    </div>
  );
}

/** A highlighted warning, optionally with one action. */
export function Callout({ message, action, onAction }: { message: ReactNode; action?: string; onAction?: () => void }) {
  return (
    <div role="alert" className="flex items-start gap-2.5 rounded-xl border border-warn/22 bg-warn/8 px-3.5 py-3 text-small leading-normal">
      <Icon name="warning" size={16} className="mt-px text-warn" />
      <div className="flex flex-1 flex-wrap items-center justify-between gap-2.5">
        <span>{message}</span>
        {action && onAction && (
          <Button small onClick={onAction}>
            {action}
          </Button>
        )}
      </div>
    </div>
  );
}
