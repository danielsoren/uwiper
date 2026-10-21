import type { ReactNode } from 'react';
import { inTauri } from '../../lib/api';
import { cn } from '../../lib/cn';
import { formatBytes } from '../../lib/format';
import { brokenEntries } from '../../store/selectors';
import { useAppStore, type Page } from '../../store/useAppStore';
import { Icon, type IconName } from '../../ui';
import { Logo } from '../brand/Logo';
import { TrafficLights } from './WindowControls';

interface NavItem {
  page: Page;
  label: string;
  icon: IconName;
  badge?: string;
}

function NavButton({ item }: { item: NavItem }) {
  const active = useAppStore((s) => s.page === item.page);
  const go = useAppStore((s) => s.go);
  return (
    <button
      type="button"
      aria-current={active ? 'page' : undefined}
      onClick={() => go(item.page)}
      className={cn(
        'group flex h-[34px] items-center gap-2.5 rounded-lg px-2.5 text-left text-body font-medium',
        active ? 'bg-accent text-white' : 'text-[#dcdce1] hover:bg-white/7',
      )}
    >
      <Icon name={item.icon} stroke={1.7} className={active ? 'text-white' : 'text-accent-soft'} />
      <span className="flex-1">{item.label}</span>
      {item.badge && <span className={cn('text-micro', active ? 'text-white/85' : 'text-fg-4')}>{item.badge}</span>}
    </button>
  );
}

function SectionLabel({ children }: { children: ReactNode }) {
  return <div className="px-2.5 pb-1 pt-3 text-micro font-semibold text-fg-4">{children}</div>;
}

function StatusCard({ tone, icon, title, text, children }: { tone: 'good' | 'warn'; icon: IconName; title: string; text: string; children?: ReactNode }) {
  return (
    <div
      className={cn(
        'mt-2.5 flex items-start gap-2.5 rounded-tile border p-3',
        tone === 'good' ? 'border-good/18 bg-good/8 text-good' : 'border-warn/22 bg-warn/8 text-warn',
      )}
    >
      <Icon name={icon} />
      <div>
        <div className="text-small font-semibold text-fg">{title}</div>
        <div className="mt-0.5 text-micro leading-[1.4] text-fg-3">{text}</div>
        {children}
      </div>
    </div>
  );
}

export function Sidebar() {
  const settings = useAppStore((s) => s.settings);
  const env = useAppStore((s) => s.env);
  const apps = useAppStore((s) => s.res.apps.data);
  const cleanup = useAppStore((s) => s.res.cleanup.data);
  const startup = useAppStore((s) => s.res.startup.data);
  const services = useAppStore((s) => s.res.services.data);
  const tasks = useAppStore((s) => s.res.tasks.data);
  const restartAsAdmin = useAppStore((s) => s.restartAsAdmin);

  const broken = brokenEntries(startup, services, tasks);
  const cleanupBytes = (cleanup ?? []).filter((i) => i.defaultOn).reduce((s, i) => s + i.sizeBytes, 0);

  const cleanUp: NavItem[] = [
    { page: 'overview', label: 'Overview', icon: 'home' },
    { page: 'apps', label: 'Apps', icon: 'apps', badge: apps ? String(apps.length) : undefined },
    { page: 'leftovers', label: 'Leftovers', icon: 'traces', badge: broken.total ? String(broken.total) : undefined },
    { page: 'cleanup', label: 'Cleanup', icon: 'sparkle', badge: cleanup ? formatBytes(cleanupBytes) : undefined },
  ];
  const manage: NavItem[] = [
    { page: 'system', label: 'System', icon: 'cpu' },
    { page: 'quarantine', label: 'Quarantine', icon: 'archive' },
  ];

  return (
    <aside aria-label="Navigation" className="scroll-thin flex flex-[0_0_236px] flex-col gap-0.5 overflow-y-auto border-r border-line-2 bg-sidebar px-3 pb-3.5 pt-3">
      {settings?.appearance.windowButtons === 'macos' ? <TrafficLights /> : <div className="h-2" data-tauri-drag-region />}
      <div className="flex items-center gap-2.5 px-2 pb-4 pt-1" data-tauri-drag-region>
        <Logo size={32} />
        <div data-tauri-drag-region>
          <div className="text-title font-bold tracking-[-0.01em]">UWiper</div>
          <div className="text-micro text-fg-4">Ultimate system cleaner</div>
        </div>
      </div>

      <SectionLabel>Clean up</SectionLabel>
      {cleanUp.map((item) => (
        <NavButton key={item.page} item={item} />
      ))}
      <SectionLabel>Manage</SectionLabel>
      {manage.map((item) => (
        <NavButton key={item.page} item={item} />
      ))}

      <div className="min-h-4 flex-1" data-tauri-drag-region />
      <NavButton item={{ page: 'settings', label: 'Settings', icon: 'sliders' }} />

      {!inTauri && <StatusCard tone="warn" icon="info" title="Browser preview" text="Sample data – run the app for real results" />}
      {inTauri && env?.elevated && (
        <StatusCard tone="good" icon="shield" title="Administrator" text="Read-only in this version – nothing is changed" />
      )}
      {inTauri && env && !env.elevated && (
        <StatusCard tone="warn" icon="lock" title="Limited mode" text="Some areas need administrator rights">
          <button
            type="button"
            onClick={restartAsAdmin}
            className="mt-2 rounded-md border border-white/14 bg-white/7 px-2 py-1 text-[11.5px] font-medium text-fg hover:bg-white/12"
          >
            Restart as administrator
          </button>
        </StatusCard>
      )}
    </aside>
  );
}
