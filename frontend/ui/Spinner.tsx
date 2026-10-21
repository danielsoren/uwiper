import { cn } from '../lib/cn';

export function Spinner({ size = 18, className }: { size?: number; className?: string }) {
  return (
    <span
      aria-hidden="true"
      style={{ width: size, height: size }}
      className={cn('inline-block flex-none animate-spin rounded-full border-2 border-white/16 border-t-accent-soft [animation-duration:.8s]', className)}
    />
  );
}

/** A centered spinner with a line of text. */
export function Loading({ label }: { label: string }) {
  return (
    <div role="status" className="flex items-center justify-center gap-3 py-12 text-fg-3">
      <Spinner />
      <span>{label}</span>
    </div>
  );
}
