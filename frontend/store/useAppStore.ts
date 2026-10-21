import { create } from 'zustand';
import { api, errorText } from '../lib/api';
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

/** Data loaded from the backend on demand and cached for the session. */
export interface Resources {
  apps: InstalledApp[];
  cleanup: CleanupItem[];
  startup: StartupItem[];
  services: ServiceItem[];
  tasks: TaskItem[];
  storage: StorageInfo;
}

export type ResourceKey = keyof Resources;

export interface Resource<T> {
  data: T | null;
  error: string | null;
  loading: boolean;
  requested: boolean;
}

const FETCH: { [K in ResourceKey]: () => Promise<Resources[K]> } = {
  apps: api.listApps,
  cleanup: api.scanCleanup,
  startup: api.listStartup,
  services: api.listServices,
  tasks: api.listTasks,
  storage: api.storageInfo,
};

const idle = <T>(): Resource<T> => ({ data: null, error: null, loading: false, requested: false });

export interface Toast {
  id: number;
  text: string;
}

interface AppState {
  page: Page;
  env: Environment | null;
  settings: Settings | null;
  res: { [K in ResourceKey]: Resource<Resources[K]> };
  toasts: Toast[];

  go: (page: Page) => void;
  init: () => void;
  /** Loads once per session; `force` loads again. */
  load: <K extends ResourceKey>(key: K, force?: boolean) => void;
  /** Applies a change right away and saves it; a failed save rolls it back. */
  updateSettings: (change: (s: Settings) => Settings) => void;
  notify: (text: string) => void;
  /** Runs a command and turns a failure into a toast. */
  attempt: (action: () => Promise<unknown>) => void;
  restartAsAdmin: () => void;
}

export const useAppStore = create<AppState>()((set, get) => {
  const patch = <K extends ResourceKey>(key: K, change: Partial<Resource<Resources[K]>>) =>
    set((s) => ({ res: { ...s.res, [key]: { ...s.res[key], ...change } } }));

  return {
    page: 'overview',
    env: null,
    settings: null,
    res: { apps: idle(), cleanup: idle(), startup: idle(), services: idle(), tasks: idle(), storage: idle() },
    toasts: [],

    go: (page) => set({ page }),

    init: () => {
      api.environment().then((env) => set({ env }), (e) => get().notify(errorText(e)));
      api.getSettings().then((settings) => set({ settings }), (e) => get().notify(errorText(e)));
    },

    load: (key, force = false) => {
      const current = get().res[key];
      if (current.loading || (current.requested && !force)) return;
      patch(key, { loading: true, requested: true, error: null });
      FETCH[key]().then(
        (data) => patch(key, { data, loading: false }),
        (e) => patch(key, { error: errorText(e), loading: false }),
      );
    },

    updateSettings: (change) => {
      const before = get().settings;
      if (!before) return;
      const next = change(before);
      set({ settings: next });
      api.saveSettings(next).then(
        () => get().load('storage', true),
        (e) => {
          get().notify(errorText(e));
          set({ settings: before });
        },
      );
    },

    notify: (text) => {
      const id = Date.now() + Math.random();
      set((s) => ({ toasts: [...s.toasts, { id, text }] }));
      setTimeout(() => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })), 4500);
    },

    attempt: (action) => {
      action().catch((e) => get().notify(errorText(e)));
    },

    restartAsAdmin: () => get().attempt(api.restartAsAdmin),
  };
});
