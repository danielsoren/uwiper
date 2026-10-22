import type { ReactNode } from 'react';
import { Icon, type IconName } from './Icon';

/** Centered placeholder for an empty list or an area that isn't ready yet. */
export function Empty({ icon, title, children, action }: { icon: IconName; title: string; children?: ReactNode; action?: ReactNode }) {
  return (
    <div className="flex flex-col items-center gap-2.5 px-5 py-14 text-center">
      <span className="grid size-16 place-items-center rounded-full bg-white/6 text-fg-3">
        <Icon name={icon} size={30} />
      </span>
      <h2 className="mt-1 text-h2 font-bold">{title}</h2>
      {children && <p className="max-w-[520px] text-body leading-[1.55] text-fg-3">{children}</p>}
      {action && <div className="mt-2 flex flex-wrap justify-center gap-2.5">{action}</div>}
    </div>
  );
}
