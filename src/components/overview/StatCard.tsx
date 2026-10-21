import { IconTile, Spinner, type IconName, type TileTone } from '../../ui';

/** A clickable summary tile on the Overview page. */
export function StatCard({
  icon,
  tone,
  value,
  label,
  sub,
  loading,
  onClick,
}: {
  icon: IconName;
  tone: TileTone;
  value: string | null;
  label: string;
  sub: string;
  loading?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex min-w-0 flex-[1_1_200px] flex-col items-start gap-2 rounded-card border border-line-2 bg-fill px-[18px] py-4 text-left hover:border-white/14 hover:bg-white/7"
    >
      <IconTile icon={icon} tone={tone} />
      <span className="mt-0.5 text-stat font-bold leading-none tracking-[-0.02em]">
        {value ?? (loading ? <Spinner size={20} /> : '–')}
      </span>
      <span className="font-medium">{label}</span>
      <span className="-mt-1.5 text-[11.5px] text-fg-4">{sub}</span>
    </button>
  );
}
