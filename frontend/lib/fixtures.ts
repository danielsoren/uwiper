// Sample data for running the UI in a plain browser (`npm run dev`).
// Never used inside the real app – there every call goes to Rust.

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

const MB = 1024 * 1024;
const HOME = 'D:\\Tools\\UWiper';

const app = (
  name: string,
  publisher: string,
  version: string,
  sizeMb: number,
  date: string,
  location: string | null,
  source: 'win32' | 'msi' = 'win32',
  scope: 'machine' | 'user' = 'machine',
): InstalledApp => ({
  id: `HKLM|${name}`,
  name,
  publisher,
  version,
  installLocation: location,
  installDate: date,
  sizeBytes: sizeMb * MB,
  reportedBytes: sizeMb * MB,
  folderBytes: location ? sizeMb * MB : null,
  icon: null,
  source,
  scope,
  arch: scope === 'user' ? null : 'x64',
  uninstallString: location ? `"${location}\\uninstall.exe"` : `MsiExec.exe /X{${name.length}A1B2C3-0000-0000-0000-000000000000}`,
  quietUninstallString: null,
  registryKey: `HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\${name}`,
});

const apps: InstalledApp[] = [
  app('7-Zip 25.01 (x64)', 'Igor Pavlov', '25.01', 6, '2026-10-11', 'C:\\Program Files\\7-Zip'),
  app('Discord', 'Discord Inc.', '1.0.9168', 512, '2026-08-02', 'C:\\Users\\you\\AppData\\Local\\Discord', 'win32', 'user'),
  app('Docker Desktop', 'Docker Inc.', '4.44.3', 3280, '2026-10-02', 'C:\\Program Files\\Docker\\Docker'),
  app('Git', 'The Git Development Community', '2.51.0', 330, '2026-09-14', 'C:\\Program Files\\Git'),
  app('Logitech G HUB', 'Logitech', '2025.6.7', 640, '2026-04-22', 'C:\\Program Files\\LGHUB'),
  app('Node.js', 'Node.js Foundation', '22.22.0', 98, '2026-07-30', 'C:\\Program Files\\nodejs\\', 'msi'),
  app('Notepad++ (64-bit x64)', 'Notepad++ Team', '8.7.9', 18, '2026-06-08', 'C:\\Program Files\\Notepad++'),
  app('NVIDIA App', 'NVIDIA Corporation', '11.0.1.163', 860, '2026-05-28', 'C:\\Program Files\\NVIDIA Corporation\\NVIDIA App'),
  app('OBS Studio', 'OBS Project', '31.1.2', 410, '2026-07-30', 'C:\\Program Files\\obs-studio'),
  app('Steam', 'Valve Corporation', '2.10.91.91', 1130, '2026-01-15', 'C:\\Program Files (x86)\\Steam'),
  app('Telegram Desktop', 'Telegram FZ-LLC', '6.1.3', 182, '2026-10-11', 'C:\\Users\\you\\AppData\\Roaming\\Telegram Desktop', 'win32', 'user'),
  app('Microsoft Visual C++ 2015-2022 Redistributable (x64)', 'Microsoft Corporation', '14.44.35211.0', 21, '2026-05-02', null, 'msi'),
];

const cleanup: CleanupItem[] = [
  { id: 'temp', group: 'windows', name: 'Temporary files', description: '%TEMP% and Windows\\Temp', defaultOn: true, sizeBytes: 1340 * MB, fileCount: 18211, paths: ['C:\\Users\\you\\AppData\\Local\\Temp', 'C:\\Windows\\Temp'] },
  { id: 'windows-update', group: 'windows', name: 'Windows Update cache', description: 'Downloaded packages for updates that are already installed', defaultOn: true, sizeBytes: 860 * MB, fileCount: 412, paths: ['C:\\Windows\\SoftwareDistribution\\Download'] },
  { id: 'crash-dumps', group: 'windows', name: 'Crash reports & memory dumps', description: 'Windows Error Reporting, minidumps', defaultOn: true, sizeBytes: 412 * MB, fileCount: 37, paths: ['C:\\ProgramData\\Microsoft\\Windows\\WER\\ReportArchive'] },
  { id: 'thumbnails', group: 'windows', name: 'Thumbnail cache', description: 'Explorer previews, rebuilt on demand', defaultOn: true, sizeBytes: 96 * MB, fileCount: 12, paths: ['C:\\Users\\you\\AppData\\Local\\Microsoft\\Windows\\Explorer\\thumbcache_256.db'] },
  { id: 'chrome', group: 'browsers', name: 'Google Chrome – cache', description: 'Cookies, passwords and history are never touched', defaultOn: true, sizeBytes: 420 * MB, fileCount: 5120, paths: ['C:\\Users\\you\\AppData\\Local\\Google\\Chrome\\User Data\\Default\\Cache'] },
  { id: 'edge', group: 'browsers', name: 'Microsoft Edge – cache', description: 'Cookies, passwords and history are never touched', defaultOn: true, sizeBytes: 180 * MB, fileCount: 2210, paths: ['C:\\Users\\you\\AppData\\Local\\Microsoft\\Edge\\User Data\\Default\\Cache'] },
  { id: 'npm', group: 'developer', name: 'npm cache', description: "Downloaded again the next time it's needed", defaultOn: false, sizeBytes: 1120 * MB, fileCount: 40210, paths: ['C:\\Users\\you\\AppData\\Local\\npm-cache'] },
  { id: 'cargo', group: 'developer', name: 'Cargo registry cache', description: 'Downloaded .crate archives', defaultOn: false, sizeBytes: 380 * MB, fileCount: 2311, paths: ['C:\\Users\\you\\.cargo\\registry\\cache'] },
  { id: 'discord', group: 'apps', name: 'Discord – cache', description: 'Images and media from chats', defaultOn: true, sizeBytes: 210 * MB, fileCount: 3011, paths: ['C:\\Users\\you\\AppData\\Roaming\\discord\\Cache'] },
];

const startup: StartupItem[] = [
  { id: '1', name: 'Discord', command: '"C:\\Users\\you\\AppData\\Local\\Discord\\Update.exe" --processStart Discord.exe', location: 'HKCU\\…\\Run', target: 'C:\\Users\\you\\AppData\\Local\\Discord\\Update.exe', fileExists: true, enabled: true, isWindows: false },
  { id: '2', name: 'Docker Desktop', command: 'C:\\Program Files\\Docker\\Docker\\Docker Desktop.exe -Autostart', location: 'HKCU\\…\\Run', target: 'C:\\Program Files\\Docker\\Docker\\Docker Desktop.exe', fileExists: true, enabled: true, isWindows: false },
  { id: '3', name: 'RazerCentral', command: '"C:\\Program Files (x86)\\Razer\\Razer Services\\Razer Central\\RazerCentral.exe" /silent', location: 'HKLM\\…\\Run (32-bit)', target: 'C:\\Program Files (x86)\\Razer\\Razer Services\\Razer Central\\RazerCentral.exe', fileExists: false, enabled: true, isWindows: false },
  { id: '4', name: 'Steam', command: '"C:\\Program Files (x86)\\Steam\\steam.exe" -silent', location: 'HKCU\\…\\Run', target: 'C:\\Program Files (x86)\\Steam\\steam.exe', fileExists: true, enabled: false, isWindows: false },
  { id: '5', name: 'SecurityHealth', command: '%windir%\\system32\\SecurityHealthSystray.exe', location: 'HKLM\\…\\Run', target: 'C:\\Windows\\system32\\SecurityHealthSystray.exe', fileExists: true, enabled: true, isWindows: true },
];

const services: ServiceItem[] = [
  { name: 'AdobeARMservice', displayName: 'Adobe Acrobat Update Service', binary: 'C:\\Program Files (x86)\\Common Files\\Adobe\\ARM\\1.0\\armsvc.exe', fileExists: false, startType: 'automatic', delayed: false, isWindows: false },
  { name: 'com.docker.service', displayName: 'Docker Desktop Service', binary: 'C:\\Program Files\\Docker\\Docker\\com.docker.service', fileExists: true, startType: 'manual', delayed: false, isWindows: false },
  { name: 'LGHUBUpdaterService', displayName: 'LGHUB Updater Service', binary: 'C:\\Program Files\\LGHUB\\lghub_updater.exe', fileExists: true, startType: 'automatic', delayed: false, isWindows: false },
  { name: 'Steam Client Service', displayName: 'Steam Client Service', binary: 'C:\\Program Files (x86)\\Common Files\\Steam\\steamservice.exe', fileExists: true, startType: 'manual', delayed: false, isWindows: false },
  { name: 'wuauserv', displayName: 'Windows Update', binary: 'C:\\Windows\\system32\\wuaueng.dll', fileExists: true, startType: 'manual', delayed: false, isWindows: true },
];

const tasks: TaskItem[] = [
  { name: 'Adobe Acrobat Update Task', folder: '', command: 'C:\\Program Files (x86)\\Common Files\\Adobe\\ARM\\1.0\\AdobeARM.exe', arguments: null, binary: 'C:\\Program Files (x86)\\Common Files\\Adobe\\ARM\\1.0\\AdobeARM.exe', fileExists: false, enabled: true, triggers: 'At log on, On a schedule', isWindows: false },
  { name: 'GoogleUpdaterTaskSystem', folder: '', command: 'C:\\Program Files (x86)\\Google\\GoogleUpdater\\updater.exe', arguments: '--wake', binary: 'C:\\Program Files (x86)\\Google\\GoogleUpdater\\updater.exe', fileExists: true, enabled: true, triggers: 'On a schedule', isWindows: false },
  { name: 'Microsoft\\Windows\\Defrag\\ScheduledDefrag', folder: 'Microsoft\\Windows\\Defrag', command: '%windir%\\system32\\defrag.exe', arguments: '-c -h -o -$', binary: 'C:\\Windows\\system32\\defrag.exe', fileExists: true, enabled: true, triggers: 'On a schedule', isWindows: true },
];

let settings: Settings = {
  storage: { oneFolder: true, quarantine: null, snapshots: null, logs: null, temp: null, logDays: 30 },
  safety: { quarantine: true, keepDays: 30, restorePoint: true, protectSystem: true, preselect: 'high-medium' },
  scan: {
    depth: 'standard',
    lookIn: { files: true, registry: true, services: true, tasks: true, drivers: true, firewall: true, shortcuts: true, path: true },
    exclusions: ['D:\\Games'],
  },
  appearance: { accent: '#0071E3', windowButtons: 'windows', translucent: true },
  footprint: { startWithWindows: false },
};

function storage(): StorageInfo {
  const dir = (key: 'quarantine' | 'snapshots' | 'logs' | 'temp') => {
    const custom = settings.storage[key];
    const path = !settings.storage.oneFolder && custom ? custom : `${HOME}\\${key}`;
    return { key, path, inside: path.toLowerCase().startsWith(`${HOME.toLowerCase()}\\`), exists: false, sizeBytes: 0 };
  };
  return {
    home: HOME,
    settingsFile: `${HOME}\\settings.toml`,
    homeSizeBytes: 41 * MB,
    dirs: [dir('quarantine'), dir('snapshots'), dir('logs'), dir('temp')],
  };
}

const environment: Environment = { elevated: false, windows: true, material: 'none', home: HOME, version: '0.1.0' };

const delay = (ms: number) => new Promise((r) => setTimeout(r, ms));

// `?delay=3000` in the URL slows the scans down, to look at loading states.
const extraDelay = Number(new URLSearchParams(window.location.search).get('delay')) || 0;

export async function fixture<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const atStart = command === 'environment' || command === 'get_settings';
  await delay(atStart ? 50 : (command === 'scan_cleanup' ? 900 : 250) + extraDelay);
  const result = ((): unknown => {
    switch (command) {
      case 'environment': return environment;
      case 'list_apps': return apps;
      case 'scan_cleanup': return cleanup;
      case 'list_startup': return startup;
      case 'list_services': return services;
      case 'list_tasks': return tasks;
      case 'get_settings': return settings;
      case 'save_settings': settings = args?.settings as Settings; return settings;
      case 'storage_info': return storage();
      default: throw new Error('Not available in the browser preview');
    }
  })();
  return structuredClone(result) as T;
}
