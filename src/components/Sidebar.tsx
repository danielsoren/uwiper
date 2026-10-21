import { inTauri } from '../lib/api';
import { formatBytes } from '../lib/format';
import { brokenEntries, useStore, type Page } from '../state/store';
import { Icon, type IconName } from './Icon';
import { Logo } from './Logo';
import { TrafficLights } from './WindowControls';

interface NavItem {
  page: Page;
  label: string;
  icon: IconName;
  badge?: string;
}

export function Sidebar() {
  const store = useStore();
  const { page, go, settings, env } = store;
  const broken = brokenEntries(store);
  const cleanupBytes = (store.cleanup.data ?? []).filter((i) => i.defaultOn).reduce((s, i) => s + i.sizeBytes, 0);

  const cleanUp: NavItem[] = [
    { page: 'overview', label: 'Overview', icon: 'home' },
    { page: 'apps', label: 'Apps', icon: 'apps', badge: store.apps.data ? String(store.apps.data.length) : undefined },
    { page: 'leftovers', label: 'Leftovers', icon: 'traces', badge: broken.total ? String(broken.total) : undefined },
    { page: 'cleanup', label: 'Cleanup', icon: 'sparkle', badge: store.cleanup.data ? formatBytes(cleanupBytes) : undefined },
  ];
  const manage: NavItem[] = [
    { page: 'system', label: 'System', icon: 'cpu' },
    { page: 'quarantine', label: 'Quarantine', icon: 'archive' },
  ];

  const link = (item: NavItem) => (
    <button
      key={item.page}
      type="button"
      className={`nav-item${page === item.page ? ' active' : ''}`}
      aria-current={page === item.page ? 'page' : undefined}
      onClick={() => go(item.page)}
    >
      <Icon name={item.icon} className="nav-icon" stroke={1.7} />
      <span className="nav-label">{item.label}</span>
      {item.badge && <span className="nav-badge">{item.badge}</span>}
    </button>
  );

  return (
    <aside className="sidebar" aria-label="Navigation">
      {settings?.appearance.windowButtons === 'macos' ? <TrafficLights /> : <div className="sidebar-drag" data-tauri-drag-region />}
      <div className="brand" data-tauri-drag-region>
        <Logo color={settings?.appearance.accent ?? '#0071E3'} />
        <div data-tauri-drag-region>
          <div className="brand-name">UWiper</div>
          <div className="brand-sub">Ultimate Wiper</div>
        </div>
      </div>

      <div className="nav-section">Clean up</div>
      {cleanUp.map(link)}
      <div className="nav-section">Manage</div>
      {manage.map(link)}

      <div className="sidebar-spacer" data-tauri-drag-region />
      {link({ page: 'settings', label: 'Settings', icon: 'sliders' })}

      {!inTauri && (
        <div className="status-card status-warn">
          <Icon name="info" size={18} />
          <div>
            <div className="status-title">Browser preview</div>
            <div className="status-text">Sample data – run the app for real results</div>
          </div>
        </div>
      )}
      {inTauri && env && (
        env.elevated ? (
          <div className="status-card status-good">
            <Icon name="shield" size={18} />
            <div>
              <div className="status-title">Administrator</div>
              <div className="status-text">Read-only in this version – nothing is changed</div>
            </div>
          </div>
        ) : (
          <div className="status-card status-warn">
            <Icon name="lock" size={18} />
            <div>
              <div className="status-title">Limited mode</div>
              <div className="status-text">Some areas need administrator rights</div>
              <button type="button" className="status-action" onClick={store.restartAsAdmin}>
                Restart as administrator
              </button>
            </div>
          </div>
        )
      )}
    </aside>
  );
}
