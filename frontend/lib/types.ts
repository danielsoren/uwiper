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
  | { kind: 'task'; path: string }
  | { kind: 'pathEntry'; scope: 'user' | 'machine'; entry: string }
  | { kind: 'driver'; inf: string };

export type Confidence = 'high' | 'medium' | 'low';

export interface Leftover {
  id: string;
  kind: 'app' | 'folder' | 'file' | 'shortcut' | 'registry' | 'service' | 'task' | 'startup' | 'environment' | 'driver';
  label: string;
  reason: string;
  confidence: Confidence;
  sizeBytes: number | null;
  /** When it last changed, as Unix time in seconds. */
  modified: number | null;
  needsAdmin: boolean;
  target: Target;
}

export interface OrphanScan {
  items: Leftover[];
  /** Scheduled tasks need administrator rights to read. */
  needsAdmin: boolean;
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
  /** In use by an app. */
  skippedFiles: number;
  /** Windows wouldn't let UWiper delete them. */
  deniedFiles: number;
  failedFiles: number;
  /** Folders left alone because they lead through a link or junction. */
  skippedLinks: number;
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
  /** Machine-wide entries need administrator rights to switch. */
  needsAdmin: boolean;
  removal: Target;
}

/** What a service can be set to; boot and system start types are left alone. */
export type ServiceStart = 'automatic' | 'delayed' | 'manual' | 'disabled';

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

export interface DriverItem {
  /** Published name, such as oem12.inf. */
  id: string;
  /** The package's own INF name, such as nv_dispi.inf. */
  inf: string | null;
  provider: string;
  class: string;
  version: string | null;
  /** yyyy-mm-dd */
  date: string | null;
  sizeBytes: number | null;
  /** A device it drives – or, for an older version, one its newest version drives. */
  device: string | null;
  inUse: boolean;
  /** A newer version of the same package is in the Driver Store. */
  old: boolean;
  /** An older version nothing uses while a device uses the newest: picked to start with. */
  suggested: boolean;
  removal: Target;
}

export interface TrackingStatus {
  active: boolean;
  startedAt: number | null;
  /** Tracking was started with administrator rights. */
  elevated: boolean;
  /** The installer's file name. */
  installer: string | null;
  /** UWiper started the installer and can tell whether it's running. */
  installerFollowed: boolean;
  installerRunning: boolean;
}

/** Something an installer added. */
export interface TraceItem {
  id: string;
  kind: Leftover['kind'];
  label: string;
  target: Target;
  /** Made at the top of its place, rather than inside something that was already there. */
  sure: boolean;
}

export interface Trace {
  id: string;
  name: string;
  /** The installed app it belongs to, as in the app list. */
  appId: string | null;
  publisher: string | null;
  version: string | null;
  installer: string | null;
  startedAt: number;
  finishedAt: number;
  /** Other apps that were installed along with it. */
  alsoInstalled: string[];
  items: TraceItem[];
}

export interface TraceView extends Trace {
  /** Its app is still installed. */
  installed: boolean;
  /** For each item: still there. */
  present: boolean[];
}

/** What tracking found, before it's saved. */
export interface TraceDraft {
  startedAt: number;
  name: string;
  appId: string | null;
  publisher: string | null;
  version: string | null;
  /** The app was already installed: this was an update. */
  updated: boolean;
  alsoInstalled: string[];
  /** Apps installed or changed meanwhile, to pick from. */
  candidates: { id: string; name: string }[];
  /** False when scheduled tasks couldn't be read. */
  tasksChecked: boolean;
  /** Tracking ran so long that nothing is ticked. */
  longWindow: boolean;
  items: (TraceItem & { sizeBytes: number | null })[];
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
  /** Which defaults the file was written with. */
  version: number;
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
