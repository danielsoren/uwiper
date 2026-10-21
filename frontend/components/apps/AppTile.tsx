import { cn } from '../../lib/cn';
import { initial, tileColor } from '../../lib/format';

/** A colored letter tile standing in for an app's icon. */
export function AppTile({ name, large }: { name: string; large?: boolean }) {
  return (
    <span
      aria-hidden="true"
      style={{ background: tileColor(name) }}
      className={cn(
        'grid flex-none place-items-center font-bold text-white',
        large
          ? 'size-14 rounded-[14px] text-[22px] shadow-[inset_0_0_0_1px_rgba(255,255,255,.14),0_6px_16px_rgba(0,0,0,.35)]'
          : 'size-[34px] rounded-[9px] text-body shadow-[inset_0_0_0_1px_rgba(255,255,255,.14)]',
      )}
    >
      {initial(name)}
    </span>
  );
}
