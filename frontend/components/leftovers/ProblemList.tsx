import type { Problem } from '../../lib/types';
import { Icon } from '../../ui';

/** Items an operation couldn't handle, with the reason for each. */
export function ProblemList({ title, problems }: { title: string; problems: Problem[] }) {
  if (problems.length === 0) return null;
  return (
    <div role="alert" className="rounded-xl border border-warn/22 bg-warn/8 px-3.5 py-3">
      <div className="mb-2 flex items-center gap-2 text-small font-semibold">
        <Icon name="warning" size={15} className="text-warn" />
        {title}
      </div>
      <ul className="flex flex-col gap-1.5 text-small">
        {problems.map((p, i) => (
          <li key={`${p.label}-${i}`} className="leading-normal">
            <span className="break-all font-mono text-fg-path">{p.label}</span>
            <span className="text-fg-3"> – {p.error}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
