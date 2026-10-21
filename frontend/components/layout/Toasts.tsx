import { useAppStore } from '../../store/useAppStore';

export function Toasts() {
  const toasts = useAppStore((s) => s.toasts);
  return (
    <div role="status" aria-live="polite" className="fixed bottom-5 right-5 z-50 flex flex-col gap-2">
      {toasts.map((t) => (
        <div
          key={t.id}
          className="max-w-[360px] rounded-tile border border-white/12 bg-[#2c2c30] px-3.5 py-2.5 text-meta shadow-[0_10px_30px_rgba(0,0,0,.45)]"
        >
          {t.text}
        </div>
      ))}
    </div>
  );
}
