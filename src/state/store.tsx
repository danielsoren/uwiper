import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { api, errorText } from '../lib/api';
import { alpha, tint } from '../lib/format';
import type {
  CleanupItem,
  Environment,
  InstalledApp,
  ServiceItem,
  Settings,
  StartupItem,
  StorageInfo,
  TaskItem,
} from '../lib/types';

export type Page = 'overview' | 'apps' | 'leftovers' | 'cleanup' | 'system' | 'quarantine' | 'settings';

export interface Resource<T> {
  data: T | null;
  error: string | null;
  loading: boolean;
  /** Loads once; `force` reloads. */
  load: (force?: boolean) => void;
}

function useResource<T>(fetcher: () => Promise<T>): Resource<T> {
  const [state, setState] = useState<{ data: T | null; error: string | null; loading: boolean }>({
    data: null,
    error: null,
    loading: false,
  });
  const started = useRef(false);
  const load = useCallback(
    (force = false) => {
      if (started.current && !force) return;
      started.current = true;
      setState((s) => ({ ...s, loading: true, error: null }));
      fetcher().then(
        (data) => setState({ data, error: null, loading: false }),
        (e) => setState((s) => ({ data: s.data, error: errorText(e), loading: false })),
      );
    },
    [fetcher],
  );
  return useMemo(() => ({ ...state, load }), [state, load]);
}

interface Toast {
  id: number;
  text: string;
}

interface Store {
  page: Page;
  go: (page: Page) => void;
  env: Environment | null;
  settings: Settings | null;
  updateSettings: (change: (s: Settings) => Settings) => void;
  storage: Resource<StorageInfo>;
  apps: Resource<InstalledApp[]>;
  cleanup: Resource<CleanupItem[]>;
  startup: Resource<StartupItem[]>;
  services: Resource<ServiceItem[]>;
  tasks: Resource<TaskItem[]>;
  restartAsAdmin: () => void;
  toasts: Toast[];
  notify: (text: string) => void;
  /** Runs a command and turns a failure into a toast. */
  attempt: (action: () => Promise<unknown>) => void;
}

const StoreContext = createContext<Store | null>(null);

export function StoreProvider({ children }: { children: ReactNode }) {
  const [page, setPage] = useState<Page>('overview');
  const [env, setEnv] = useState<Environment | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [toasts, setToasts] = useState<Toast[]>([]);

  const storage = useResource(api.storageInfo);
  const apps = useResource(api.listApps);
  const cleanup = useResource(api.scanCleanup);
  const startup = useResource(api.listStartup);
  const services = useResource(api.listServices);
  const tasks = useResource(api.listTasks);

  const notify = useCallback((text: string) => {
    const id = Date.now() + Math.random();
    setToasts((t) => [...t, { id, text }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), 4500);
  }, []);

  const attempt = useCallback(
    (action: () => Promise<unknown>) => {
      action().catch((e) => notify(errorText(e)));
    },
    [notify],
  );

  useEffect(() => {
    api.environment().then(setEnv, (e) => notify(errorText(e)));
    api.getSettings().then(setSettings, (e) => notify(errorText(e)));
  }, [notify]);

  // Saves go out in order; a failed save restores what was there before.
  const settingsRef = useRef<Settings | null>(null);
  settingsRef.current = settings;
  const storageLoad = storage.load;
  const updateSettings = useCallback(
    (change: (s: Settings) => Settings) => {
      const current = settingsRef.current;
      if (!current) return;
      const next = change(current);
      settingsRef.current = next;
      setSettings(next);
      api.saveSettings(next).then(
        () => storageLoad(true),
        (e) => {
          notify(errorText(e));
          settingsRef.current = current;
          setSettings(current);
        },
      );
    },
    [notify, storageLoad],
  );

  const restartAsAdmin = useCallback(() => attempt(api.restartAsAdmin), [attempt]);

  // Accent color and window material drive a few CSS variables.
  const accent = settings?.appearance.accent ?? '#0071E3';
  const translucent = (settings?.appearance.translucent ?? true) && env?.material === 'mica';
  useEffect(() => {
    const root = document.documentElement;
    root.style.setProperty('--accent', accent);
    root.style.setProperty('--accent-soft', tint(accent, 0.4));
    root.style.setProperty('--accent-tile', alpha(accent, 0.2));
    root.dataset.material = translucent ? 'mica' : 'solid';
  }, [accent, translucent]);

  const value = useMemo<Store>(
    () => ({
      page,
      go: setPage,
      env,
      settings,
      updateSettings,
      storage,
      apps,
      cleanup,
      startup,
      services,
      tasks,
      restartAsAdmin,
      toasts,
      notify,
      attempt,
    }),
    [page, env, settings, updateSettings, storage, apps, cleanup, startup, services, tasks, restartAsAdmin, toasts, notify, attempt],
  );

  return <StoreContext.Provider value={value}>{children}</StoreContext.Provider>;
}

export function useStore(): Store {
  const store = useContext(StoreContext);
  if (!store) throw new Error('useStore outside StoreProvider');
  return store;
}

/** Entries that point to files that are gone – the leftovers we can see today. */
export function brokenEntries(store: Pick<Store, 'startup' | 'services' | 'tasks'>) {
  const startup = (store.startup.data ?? []).filter((i) => i.fileExists === false);
  const services = (store.services.data ?? []).filter((s) => s.fileExists === false && !s.isWindows);
  const tasks = (store.tasks.data ?? []).filter((t) => t.fileExists === false && !t.isWindows);
  return { startup, services, tasks, total: startup.length + services.length + tasks.length };
}
