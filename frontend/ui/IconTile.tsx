import { cn } from '../lib/cn';
import { Icon, type IconName } from './Icon';

export type TileTone = 'accent' | 'orange' | 'purple' | 'green' | 'neutral';

const TONE: Record<TileTone, string> = {
  accent: 'bg-accent-tile text-accent-soft',
  orange: 'bg-warn/16 text-warn-fg',
  purple: 'bg-[#bf5af2]/16 text-purple-fg',
  green: 'bg-good/14 text-good-fg',
  neutral: 'bg-white/8 text-fg-2',
};

/** A tinted rounded square holding an icon. */
export function IconTile({ icon, tone = 'accent', large, small }: { icon: IconName; tone?: TileTone; large?: boolean; small?: boolean }) {
  return (
    <span
      className={cn(
        'grid flex-none place-items-center',
        large ? 'size-[46px] rounded-xl' : 'size-[34px]',
        small ? 'rounded-[9px]' : 'rounded-tile',
        TONE[tone],
      )}
    >
      <Icon name={icon} size={large ? 22 : small ? 17 : 18} />
    </span>
  );
}
