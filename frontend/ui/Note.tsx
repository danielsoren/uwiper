import type { ReactNode } from 'react';
import { cn } from '../lib/cn';
import { Button } from './Button';
import { Icon, type IconName } from './Icon';

/** A quiet line of explanation with an icon. */
export function Note({ icon = 'info', children }: { icon?: IconName; children: ReactNode }) {
  return (
    <div className="flex items-start gap-2.5 px-[15px] text-small leading-normal text-fg-3 [&_strong]:font-semibold [&_strong]:text-fg-2">
      <Icon name={icon} size={16} className="mt-px" />
      <div>{children}</div>
    </div>
  );
}

/** A highlighted warning, optionally with one action. With an action the first line of text sits on the
 * button's centre line, and so does the icon. */
export function Callout({ message, action, onAction }: { message: ReactNode; action?: string; onAction?: () => void }) {
  const hasAction = !!(action && onAction);
  return (
    <div role="alert" className="flex items-start gap-2.5 rounded-xl border border-warn/22 bg-warn/8 px-3.5 py-3 text-small leading-normal">
      <Icon name="warning" size={16} className={cn('text-warn', hasAction ? 'mt-[7px]' : 'mt-px')} />
      <div className="flex flex-1 flex-wrap items-start justify-between gap-x-2.5 gap-y-1">
        <span className={cn(hasAction && 'py-1.5')}>{message}</span>
        {hasAction && (
          <Button small onClick={onAction}>
            {action}
          </Button>
        )}
      </div>
    </div>
  );
}
