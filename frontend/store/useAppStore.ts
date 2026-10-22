import { create } from 'zustand';
import { api, errorText } from '../lib/api';
import type {
  Batch,
  CleanupItem,
  DriverItem,
  OrphanScan,
  Environment,
  InstalledApp,
  ServiceItem,
  Settings,
  StartupItem,
  StorageInfo,
  TaskItem,
  TraceView,
  TrackingStatus,
} from '../lib/types';

export type Page = 'overview' | 'apps' | 'leftovers' | 'cleanup' | 'system' | 'installs' | 'quarantine' | 'settings';

/** Data loaded from the backend on demand and cached for the session. */
export interface Resources {
  apps: InstalledApp[];
  cleanup: CleanupItem[];
  startup: StartupItem[];
  services: ServiceItem[];
  tasks: TaskItem[];
  drivers: DriverItem[];
  traces: TraceView[];
  tracking: TrackingStatus;
  storage: StorageInfo;
  quarantine: Batch[];
  orphans: OrphanScan;
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
  drivers: api.listDrivers,
  traces: api.listTraces,
  tracking: api.trackingStatus,
  storage: api.storageInfo,
  quarantine: api.listQuarantine,
  orphans: api.scanOrphans,
};

const idle = <T>(): Resource<T> => ({ data: null, error: null, loading: false, requested: false });

export interface Toast {
  id: number;
  text: string;
}

interface AppState {
  page: Page;
  /** A section or tab the page should bring into view, set by `go`. */
  focus: string | null;
  /** False until the first data has loaded; the splash screen shows meanwhile. */
  ready: boolean;
  env: Environment | null;
  settings: Settings | null;
  res: { [K in ResourceKey]: Resource<Resources[K]> };
  toasts: Toast[];

  go: (page: Page, focus?: string) => void;
  init: () => void;
  /** Loads once per session; `force` loads again. */
  load: <K extends ResourceKey>(key: K, force?: boolean) => void;
  /** Loads again whatever of these was loaded before, after something changed. */
  refresh: (...keys: ResourceKey[]) => void;
  /** After anything was removed or restored: every list it can show up in. */
  afterChange: () => void;
  /** Changes loaded data in place, e.g. to show a switch flipped before the backend confirms it. */
  update: <K extends ResourceKey>(key: K, change: (data: Resources[K]) => Resources[K]) => void;
  /** Applies a change right away and saves it; a failed save rolls it back. */
  updateSettings: (change: (s: Settings) => Settings) => void;
  /** Shows a toast for `ms` (4.5 s by default). */
  notify: (text: string, ms?: number) => void;
  /** Runs a command and turns a failure into a toast. */
  attempt: (action: () => Promise<unknown>) => void;
  restartAsAdmin: () => void;
}

export const useAppStore = create<AppState>()((set, get) => {
  let started = false;
  const patch = <K extends ResourceKey>(key: K, change: Partial<Resource<Resources[K]>>) =>
    set((s) => ({ res: { ...s.res, [key]: { ...s.res[key], ...change } } }));

  return {
    page: 'overview',
    focus: null,
    ready: false,
    env: null,
    settings: null,
    res: {
      apps: idle(),
      cleanup: idle(),
      startup: idle(),
      services: idle(),
      tasks: idle(),
      drivers: idle(),
      traces: idle(),
      tracking: idle(),
      storage: idle(),
      quarantine: idle(),
      orphans: idle(),
    },
    toasts: [],

    go: (page, focus) => set({ page, focus: focus ?? null }),

    init: () => {
      if (started) return;
      started = true;
      const env = api.environment().then((env) => set({ env }), (e) => get().notify(errorText(e)));
      const settings = api.getSettings().then((settings) => set({ settings }), (e) => get().notify(errorText(e)));
      // Settings that couldn't be read, or a choice that changed with this version.
      api.settingsNotice().then((text) => text && get().notify(text, 15000), () => undefined);
      // Settings shows these numbers, so measure them before the first screen.
      get().load('storage');
      // The sidebar shows whether an install is being tracked, and Apps which apps were.
      get().load('tracking');
      get().load('traces');
      Promise.all([env, settings]).then(() => set({ ready: true }));
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

    refresh: (...keys) => {
      for (const key of keys) if (get().res[key].requested) get().load(key, true);
    },

    update: (key, change) => {
      const data = get().res[key].data;
      if (data) patch(key, { data: change(data) });
    },

    afterChange: () => get().refresh('quarantine', 'storage', 'startup', 'services', 'tasks', 'drivers', 'traces', 'orphans'),

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

    notify: (text, ms = 4500) => {
      const id = Date.now() + Math.random();
      set((s) => ({ toasts: [...s.toasts, { id, text }] }));
      setTimeout(() => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })), ms);
    },

    attempt: (action) => {
      action().catch((e) => get().notify(errorText(e)));
    },

    restartAsAdmin: () => get().attempt(api.restartAsAdmin),
  };
});
