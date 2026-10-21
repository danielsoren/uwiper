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

/** Something UWiper can remove; the backend checks it against the safety rules. */
export type Target =
  | { kind: 'path'; path: string }
  | { kind: 'registryKey'; key: string }
  | { kind: 'registryValue'; key: string; name: string }
  | { kind: 'service'; name: string }
  | { kind: 'task'; path: string };

export type Confidence = 'high' | 'medium' | 'low';

export interface Leftover {
  id: string;
  kind: 'folder' | 'file' | 'shortcut' | 'registry' | 'service' | 'task' | 'startup';
  label: string;
  reason: string;
  confidence: Confidence;
  sizeBytes: number | null;
  needsAdmin: boolean;
  target: Target;
}

/** What the leftover scan needs to know about the app it follows. */
export type AppRef = Pick<InstalledApp, 'id' | 'name' | 'publisher' | 'installLocation'>;

export interface Removal {
  label: string;
  target: Target;
  sizeBytes?: number;
}

export interface Problem {
  label: string;
  error: string;
}

export interface RemoveReport {
  batchId: string | null;
  removed: number;
  freedBytes: number;
  problems: Problem[];
  notes: string[];
}

export interface RestoreReport {
  restored: number;
  problems: Problem[];
  restartNeeded: boolean;
}

export interface QuarantineItem {
  label: string;
  target: Target;
  sizeBytes: number;
  note: string | null;
}

export interface Batch {
  id: string;
  title: string;
  /** Unix time in seconds */
  createdAt: number;
  items: QuarantineItem[];
}

export interface CleanReport {
  freedBytes: number;
  skippedFiles: number;
}

export interface RestorePoint {
  created: boolean;
  message: string;
}

export type CleanupGroup = 'windows' | 'browsers' | 'developer' | 'apps';

export interface CleanupItem {
  id: string;
  group: CleanupGroup;
  name: string;
  description: string;
  defaultOn: boolean;
  /** Found by folder name rather than from UWiper's list of known caches. */
  detected: boolean;
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
  removal: Target;
}

export interface ServiceItem {
  name: string;
  displayName: string;
  binary: string | null;
  fileExists: boolean | null;
  startType: 'boot' | 'system' | 'automatic' | 'manual' | 'disabled';
  delayed: boolean;
  isWindows: boolean;
  removal: Target;
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
  removal: Target;
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
}
