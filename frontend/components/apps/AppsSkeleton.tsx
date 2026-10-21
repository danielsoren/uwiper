import { Skeleton } from '../../ui';

const WIDTHS = ['w-2/5', 'w-1/2', 'w-1/3', 'w-3/5', 'w-[45%]'];

/** The Apps split view in outline while apps are being measured. */
export function AppsSkeleton() {
  return (
    <div role="status" aria-label="Measuring installed apps" className="flex min-h-0 flex-1 border-t border-line">
      <div className="min-w-0 flex-[3_1_380px] overflow-hidden border-r border-line pb-3 pl-4 pr-2.5 pt-2">
        {Array.from({ length: 12 }, (_, i) => (
          <div key={i} className="mb-0.5 flex min-h-[54px] items-center gap-3 px-3 py-2">
            <Skeleton className="size-[34px] rounded-[9px]" />
            <div className="flex flex-1 flex-col gap-2">
              <Skeleton className={WIDTHS[i % WIDTHS.length]} />
              <Skeleton className="h-2.5 w-1/4" />
            </div>
            <Skeleton className="w-12" />
          </div>
        ))}
      </div>
      <div className="min-w-0 flex-[2_1_360px] px-[26px] pt-5">
        <div className="flex items-center gap-3.5">
          <Skeleton className="size-14 rounded-[14px]" />
          <div className="flex flex-1 flex-col gap-2.5">
            <Skeleton className="h-4 w-1/2" />
            <Skeleton className="w-1/3" />
          </div>
        </div>
        <Skeleton className="mt-[18px] h-[188px] rounded-xl" />
      </div>
    </div>
  );
}
