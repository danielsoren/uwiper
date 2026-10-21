// Mirrors the structs the Rust commands return (camelCase on the wire).

export interface InstalledApp {
  id: string;
  name: string;
  publisher: string | null;
  version: string | null;
  installLocation: string | null;
  installDate: string | null;
  /** The measured folder or the installer's figure, whichever is larger. */
  sizeBytes: number | null;
  reportedBytes: number | null;
  folderBytes: number | null;
  /** PNG data URL, when the app has an icon UWiper could read. */
  icon: string | null;
  source: 'win32' | 'msi';
  scope: 'machine' | 'user';
  arch: 'x64' | 'x86' | null;
  uninstallString: string | null;
  quietUninstallString: string | null;
  registryKey: string;
}

export type CleanupGroup = 'windows' | 'browsers' | 'developer' | 'apps';

export interface CleanupItem {
  id: string;
  group: CleanupGroup;
  name: string;
  description: string;
  defaultOn: boolean;
  sizeBytes: number;
  fileCount: number;
  paths: string[];
}

export interface StartupItem {
  id: string;
  name: string;
  command: string;
  location: string;
  target: string | null;
  fileExists: boolean | null;
  enabled: boolean;
  isWindows: boolean;
}

export interface ServiceItem {
  name: string;
  displayName: string;
  binary: string | null;
  fileExists: boolean | null;
  startType: 'boot' | 'system' | 'automatic' | 'manual' | 'disabled';
  delayed: boolean;
  isWindows: boolean;
}

export interface TaskItem {
  name: string;
  folder: string;
  command: string | null;
  arguments: string | null;
  binary: string | null;
  fileExists: boolean | null;
  enabled: boolean;
  triggers: string;
  isWindows: boolean;
}

export type DataDirKey = 'quarantine' | 'snapshots' | 'logs' | 'temp';

export interface DataDir {
  key: DataDirKey;
  path: string;
  inside: boolean;
  exists: boolean;
  sizeBytes: number;
}

export interface StorageInfo {
  home: string;
  settingsFile: string;
  homeSizeBytes: number;
  dirs: DataDir[];
}

export interface Environment {
  elevated: boolean;
  windows: boolean;
  material: 'mica' | 'none';
  home: string;
  version: string;
}

export interface Settings {
  storage: {
    oneFolder: boolean;
    quarantine: string | null;
    snapshots: string | null;
    logs: string | null;
    temp: string | null;
    logDays: number;
  };
  safety: {
    quarantine: boolean;
    keepDays: number;
    restorePoint: boolean;
    protectSystem: boolean;
    preselect: 'high' | 'high-medium' | 'all';
  };
  scan: {
    depth: 'quick' | 'standard' | 'deep';
    lookIn: {
      files: boolean;
      registry: boolean;
      services: boolean;
      tasks: boolean;
      drivers: boolean;
      firewall: boolean;
      shortcuts: boolean;
      path: boolean;
    };
    exclusions: string[];
  };
  appearance: {
    accent: string;
    windowButtons: 'windows' | 'macos';
    translucent: boolean;
  };
  footprint: {
    startWithWindows: boolean;
  };
}
