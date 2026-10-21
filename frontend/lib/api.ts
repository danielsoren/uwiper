import { invoke } from '@tauri-apps/api/core';
import type {
  CleanupItem,
  Environment,
  InstalledApp,
  ServiceItem,
  Settings,
  StartupItem,
  StorageInfo,
  TaskItem,
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
  getSettings: () => call<Settings>('get_settings'),
  saveSettings: (settings: Settings) => call<Settings>('save_settings', { settings }),
  storageInfo: () => call<StorageInfo>('storage_info'),
  restartAsAdmin: () => call<void>('restart_as_admin'),
  openFolder: (path: string) => call<void>('open_folder', { path }),
  revealPath: (path: string) => call<void>('reveal_path', { path }),
};

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
