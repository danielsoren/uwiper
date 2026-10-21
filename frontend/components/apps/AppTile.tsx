import { useState } from 'react';
import { cn } from '../../lib/cn';
import { initial, tileColor } from '../../lib/format';

/** The app's own icon, or a colored letter tile when it has none. */
export function AppTile({ name, icon, large }: { name: string; icon?: string | null; large?: boolean }) {
  const [broken, setBroken] = useState(false);
  const size = large ? 'size-14' : 'size-[34px]';

  if (icon && !broken) {
    return <img src={icon} alt="" draggable={false} onError={() => setBroken(true)} className={cn(size, 'flex-none object-contain')} />;
  }
  return (
    <span
      aria-hidden="true"
      style={{ background: tileColor(name) }}
      className={cn(
        size,
        'grid flex-none place-items-center font-bold text-white',
        large
          ? 'rounded-[14px] text-[22px] shadow-[inset_0_0_0_1px_rgba(255,255,255,.14),0_6px_16px_rgba(0,0,0,.35)]'
          : 'rounded-[9px] text-body shadow-[inset_0_0_0_1px_rgba(255,255,255,.14)]',
      )}
    >
      {initial(name)}
    </span>
  );
}
