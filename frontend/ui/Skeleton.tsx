import { cn } from '../lib/cn';
import { ListCard } from './List';

/** A shimmering placeholder in the shape of content that is still loading. */
export function Skeleton({ className }: { className?: string }) {
  return <span aria-hidden="true" className={cn('skeleton block h-3 rounded-md', className)} />;
}

const WIDTHS = ['w-2/5', 'w-1/2', 'w-1/3', 'w-3/5', 'w-[45%]'];

/** Placeholder rows shaped like a ListCard of ListRows. */
export function SkeletonRows({ rows = 5, tile = true }: { rows?: number; tile?: boolean }) {
  return (
    <div role="status" aria-label="Loading">
      <ListCard>
        {Array.from({ length: rows }, (_, i) => (
          <div key={i} className="flex min-h-[60px] items-center gap-3.5 px-4 py-2.5">
            {tile && <Skeleton className="size-[34px] rounded-[9px]" />}
            <div className="flex flex-1 flex-col gap-2">
              <Skeleton className={WIDTHS[i % WIDTHS.length]} />
              <Skeleton className="h-2.5 w-1/4" />
            </div>
            <Skeleton className="h-5 w-16 rounded-full" />
          </div>
        ))}
      </ListCard>
    </div>
  );
}
