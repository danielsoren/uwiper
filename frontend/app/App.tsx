import { useEffect, type ComponentType } from 'react';
import { Sidebar } from '../components/layout/Sidebar';
import { Splash } from '../components/layout/Splash';
import { Toasts } from '../components/layout/Toasts';
import { CaptionButtons } from '../components/layout/WindowControls';
import { Apps } from '../pages/Apps';
import { Cleanup } from '../pages/Cleanup';
import { Leftovers } from '../pages/Leftovers';
import { Overview } from '../pages/Overview';
import { Quarantine } from '../pages/Quarantine';
import { Settings } from '../pages/Settings';
import { System } from '../pages/System';
import { useThemeSync } from '../store/hooks';
import { useAppStore, type Page } from '../store/useAppStore';

const PAGES: Record<Page, ComponentType> = {
  overview: Overview,
  apps: Apps,
  leftovers: Leftovers,
  cleanup: Cleanup,
  system: System,
  quarantine: Quarantine,
  settings: Settings,
};

export function App() {
  const page = useAppStore((s) => s.page);
  const ready = useAppStore((s) => s.ready);
  const init = useAppStore((s) => s.init);
  const macButtons = useAppStore((s) => s.settings?.appearance.windowButtons === 'macos');
  useEffect(init, [init]);
  useThemeSync();

  const Current = PAGES[page];
  return (
    <div className="relative flex h-full">
      {!macButtons && <CaptionButtons />}
      {ready ? (
        <div className="fade-in flex min-w-0 flex-1">
          <Sidebar />
          <main className="flex min-w-0 flex-1 flex-col bg-main">
            {/* Title-bar strip: room for the caption buttons and a place to drag the window. */}
            <div className="h-8 flex-none" data-tauri-drag-region />
            <Current />
          </main>
        </div>
      ) : (
        <Splash />
      )}
      <Toasts />
    </div>
  );
}
