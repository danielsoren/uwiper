import { Icon, type IconName } from './Icon';

/** A borderless square button with just an icon. */
export function IconButton({ icon, label, onClick }: { icon: IconName; label: string; onClick: () => void }) {
  return (
    <button
      type="button"
      aria-label={label}
      onClick={onClick}
      className="grid size-7 flex-none place-items-center rounded-[7px] text-fg-3 hover:bg-hover hover:text-fg"
    >
      <Icon name={icon} size={14} stroke={2.2} />
    </button>
  );
}
