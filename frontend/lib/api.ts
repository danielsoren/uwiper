import { invoke } from '@tauri-apps/api/core';
import type {
  AppRef,
  Batch,
  CleanReport,
  CleanupItem,
  DriverItem,
  Environment,
  InstalledApp,
  Leftover,
  OrphanScan,
  Removal,
  RemoveReport,
  RestorePoint,
  RestoreReport,
  ServiceItem,
  ServiceStart,
  Settings,
  StartupItem,
  StorageInfo,
  TaskItem,
  Trace,
  TraceDraft,
  TraceView,
  TrackingStatus,
} from './types';

/** False when the UI runs in a plain browser (`npm run dev`) for design work. */
export const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (inTauri) return invoke<T>(command, args);
  const { fixture } = await import('./fixtures');
  return fixture<T>(command, args);
}

export const api = {
  environment: () => call<Environment>('environment'),
  listApps: () => call<InstalledApp[]>('list_apps'),
  scanCleanup: () => call<CleanupItem[]>('scan_cleanup'),
  listStartup: () => call<StartupItem[]>('list_startup'),
  listServices: () => call<ServiceItem[]>('list_services'),
  listTasks: () => call<TaskItem[]>('list_tasks'),
  listDrivers: () => call<DriverItem[]>('list_drivers'),
  setStartupEnabled: (id: string, enabled: boolean) => call<void>('set_startup_enabled', { id, enabled }),
  setServiceStart: (name: string, start: ServiceStart) => call<void>('set_service_start', { name, start }),
  setTaskEnabled: (name: string, enabled: boolean) => call<void>('set_task_enabled', { name, enabled }),
  getSettings: () => call<Settings>('get_settings'),
  saveSettings: (settings: Settings) => call<Settings>('save_settings', { settings }),
  storageInfo: () => call<StorageInfo>('storage_info'),
  restartAsAdmin: () => call<void>('restart_as_admin'),
  openFolder: (path: string) => call<void>('open_folder', { path }),
  revealPath: (path: string) => call<void>('reveal_path', { path }),

  createRestorePoint: (appName: string) => call<RestorePoint>('create_restore_point', { appName }),
  startUninstall: (id: string, quiet: boolean) => call<void>('start_uninstall', { id, quiet }),
  uninstallFinished: (id: string) => call<boolean>('uninstall_finished', { id }),
  scanLeftovers: (app: AppRef) => call<Leftover[]>('scan_leftovers', { app }),
  scanOrphans: () => call<OrphanScan>('scan_orphans'),
  removeItems: (title: string, items: Removal[]) => call<RemoveReport>('remove_items', { title, items }),
  listQuarantine: () => call<Batch[]>('list_quarantine'),
  restoreBatch: (id: string) => call<RestoreReport>('restore_batch', { id }),
  deleteBatch: (id: string) => call<void>('delete_batch', { id }),
  cleanCaches: (ids: string[]) => call<CleanReport>('clean_caches', { ids }),

  trackingStatus: () => call<TrackingStatus>('tracking_status'),
  startTracking: () => call<TrackingStatus>('start_tracking'),
  launchInstaller: (path: string) => call<void>('launch_installer', { path }),
  finishTracking: () => call<TraceDraft>('finish_tracking'),
  saveTrace: (keep: string[]) => call<Trace>('save_trace', { keep }),
  cancelTracking: () => call<void>('cancel_tracking'),
  listTraces: () => call<TraceView[]>('list_traces'),
  deleteTrace: (id: string) => call<void>('delete_trace', { id }),
  traceLeftovers: (id: string) => call<Leftover[]>('trace_leftovers', { id }),
};

/** Asks for an installer to track. The browser preview pretends one was picked. */
export async function pickInstaller(): Promise<string | null> {
  if (!inTauri) return 'C:\\Users\\you\\Downloads\\Obsidian-1.7.7.exe';
  const { open } = await import('@tauri-apps/plugin-dialog');
  const picked = await open({ multiple: false, title: 'Choose an installer', filters: [{ name: 'Installers', extensions: ['exe', 'msi'] }] });
  return typeof picked === 'string' ? picked : null;
}

export async function pickFolder(title: string): Promise<string | null> {
  if (!inTauri) return null;
  const { open } = await import('@tauri-apps/plugin-dialog');
  const picked = await open({ directory: true, multiple: false, title });
  return typeof picked === 'string' ? picked : null;
}

export async function windowAction(action: 'minimize' | 'toggleMaximize' | 'close') {
  if (!inTauri) return;
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  const win = getCurrentWindow();
  if (action === 'minimize') await win.minimize();
  else if (action === 'toggleMaximize') await win.toggleMaximize();
  else await win.close();
}

export function errorText(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  return 'Something went wrong';
}
