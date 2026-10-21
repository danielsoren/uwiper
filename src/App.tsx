import type { ComponentType } from 'react';
import { Sidebar } from './components/Sidebar';
import { CaptionButtons } from './components/WindowControls';
import { Apps } from './pages/Apps';
import { Cleanup } from './pages/Cleanup';
import { Leftovers } from './pages/Leftovers';
import { Overview } from './pages/Overview';
import { Quarantine } from './pages/Quarantine';
import { Settings } from './pages/Settings';
import { System } from './pages/System';
import { StoreProvider, useStore, type Page } from './state/store';

const PAGES: Record<Page, ComponentType> = {
  overview: Overview,
  apps: Apps,
  leftovers: Leftovers,
  cleanup: Cleanup,
  system: System,
  quarantine: Quarantine,
  settings: Settings,
};

function Shell() {
  const { page, settings, toasts } = useStore();
  const Current = PAGES[page];
  const macButtons = settings?.appearance.windowButtons === 'macos';
  return (
    <div className={`app${macButtons ? ' mac-buttons' : ''}`}>
      {!macButtons && <CaptionButtons />}
      <Sidebar />
      <main className="main">
        <Current />
      </main>
      <div className="toasts" role="status" aria-live="polite">
        {toasts.map((t) => (
          <div key={t.id} className="toast">
            {t.text}
          </div>
        ))}
      </div>
    </div>
  );
}

export function App() {
  return (
    <StoreProvider>
      <Shell />
    </StoreProvider>
  );
}
