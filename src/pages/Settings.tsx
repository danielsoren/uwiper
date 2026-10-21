import { PageBody, PageHeader } from '../components/layout/Page';
import { DataDirRow } from '../components/settings/DataDirRow';
import { ExclusionList } from '../components/settings/ExclusionList';
import { Locked, SettingsRow, SettingsSection } from '../components/settings/SettingsSection';
import { api } from '../lib/api';
import { formatBytes, plural } from '../lib/format';
import type { Settings as SettingsT } from '../lib/types';
import { useResource } from '../store/hooks';
import { useAppStore } from '../store/useAppStore';
import { Button, Card, Chip, IconTile, Loading, Pill, Segmented, Swatch, Switch } from '../ui';

const ACCENTS: [string, string][] = [
  ['#0071E3', 'Blue'],
  ['#8E44EC', 'Purple'],
  ['#D6336C', 'Pink'],
  ['#C2410C', 'Orange'],
];

const LOOK_IN: [keyof SettingsT['scan']['lookIn'], string][] = [
  ['files', 'Files & folders'],
  ['registry', 'Registry'],
  ['services', 'Services'],
  ['tasks', 'Scheduled tasks'],
  ['drivers', 'Drivers'],
  ['firewall', 'Firewall rules'],
  ['shortcuts', 'Shortcuts'],
  ['path', 'PATH'],
];

const days = (...values: number[]) => values.map((d) => ({ value: String(d), label: `${d} days` }));

export function Settings() {
  const settings = useAppStore((s) => s.settings);
  const env = useAppStore((s) => s.env);
  const updateSettings = useAppStore((s) => s.updateSettings);
  const attempt = useAppStore((s) => s.attempt);
  const storage = useResource('storage');

  if (!settings) {
    return (
      <>
        <PageHeader title="Settings" />
        <PageBody>
          <Loading label="Loading settings…" />
        </PageBody>
      </>
    );
  }

  const set = <K extends keyof SettingsT>(section: K, patch: Partial<SettingsT[K]>) =>
    updateSettings((s) => ({ ...s, [section]: { ...s[section], ...patch } }));

  const info = storage.data;
  const outsideDirs = (info?.dirs ?? []).filter((d) => !d.inside).length;
  const outside = outsideDirs + (settings.footprint.startWithWindows ? 1 : 0);
  const { storage: st, safety, scan, appearance, footprint } = settings;

  return (
    <>
      <PageHeader title="Settings" subtitle="Changes are saved right away" />
      <PageBody narrow>
        <Card padded className="flex flex-wrap items-center gap-4">
          <IconTile icon="folder" large />
          <div className="min-w-0 flex-1">
            <h2 className="text-title font-semibold">
              {outsideDirs ? `UWiper folder plus ${plural(outsideDirs, 'custom location', 'custom locations')}` : 'Everything UWiper has is in one folder'}
            </h2>
            <div className="mt-1 select-text break-all font-mono text-small text-fg-path">{info?.home ?? env?.home ?? '…'}</div>
            <div className="text-small text-fg-3">
              {info ? `${formatBytes(info.homeSizeBytes)} inside` : 'Measuring…'} ·{' '}
              {outside === 0 ? 'nothing anywhere else' : `${plural(outside, 'thing', 'things')} elsewhere, all listed below`}
            </div>
          </div>
          {info && (
            <Button small icon="folder" onClick={() => attempt(() => api.openFolder(info.home))}>
              Open
            </Button>
          )}
        </Card>

        <SettingsSection title="Storage" note="You decide where UWiper keeps its data. By default it never writes to AppData, ProgramData or the registry.">
          <SettingsRow
            label="Keep everything in the UWiper folder"
            sub={st.oneFolder ? 'Quarantine, snapshots, logs and temp files stay next to the app' : 'Off – pick a location for each one below'}
            control={<Switch checked={st.oneFolder} onChange={(v) => set('storage', { oneFolder: v })} label="Keep everything in the UWiper folder" />}
          />
          <SettingsRow
            label="Settings file"
            sub="Plain text, safe to back up or copy to another PC"
            mono={info?.settingsFile}
            control={<Locked>Always next to UWiper</Locked>}
          />
          {(info?.dirs ?? []).map((dir) => (
            <DataDirRow key={dir.key} dir={dir} editable={!st.oneFolder} onSave={(path) => set('storage', { [dir.key]: path })} />
          ))}
          <SettingsRow
            label="Keep logs for"
            control={<Segmented label="Keep logs for" value={String(st.logDays)} onChange={(v) => set('storage', { logDays: Number(v) })} options={days(7, 30, 90)} />}
          />
        </SettingsSection>

        <SettingsSection title="Outside the UWiper folder" note="Every place UWiper writes anywhere else. Nothing else is ever created.">
          <SettingsRow
            label="Start with Windows"
            sub="Off by default – UWiper doesn't need to run in the background"
            mono="HKCU\…\CurrentVersion\Run\UWiper"
            control={<Switch checked={footprint.startWithWindows} onChange={(v) => set('footprint', { startWithWindows: v })} label="Start with Windows" />}
          />
          <SettingsRow
            label="WebView data"
            sub="The browser engine's profile, kept inside the UWiper folder"
            mono={info ? `${info.home}\\webview` : undefined}
            control={<Pill tone="good">Inside</Pill>}
          />
          <SettingsRow label="Start menu shortcut" sub="UWiper is portable and doesn't add one" control={<Pill>Not created</Pill>} />
          <SettingsRow label={'"Installed apps" entry'} sub="Not needed for a portable app – delete the folder to remove it" control={<Pill>Not created</Pill>} />
        </SettingsSection>

        <SettingsSection title="Safety" note="Used by uninstall and cleanup, which arrive in v0.2.">
          <SettingsRow
            label="Quarantine"
            sub={safety.quarantine ? 'Keep everything UWiper removes so it can be put back' : 'Off – removed items are deleted for good right away'}
            warn={!safety.quarantine}
            control={<Switch checked={safety.quarantine} onChange={(v) => set('safety', { quarantine: v })} label="Quarantine" />}
          />
          <SettingsRow
            label="Keep items for"
            sub="After that, a batch is deleted for good"
            control={<Segmented label="Keep items for" value={String(safety.keepDays)} onChange={(v) => set('safety', { keepDays: Number(v) })} options={days(7, 14, 30, 90)} />}
          />
          <SettingsRow
            label="Restore point before uninstalling"
            sub="Windows System Restore, created automatically"
            control={<Switch checked={safety.restorePoint} onChange={(v) => set('safety', { restorePoint: v })} label="Restore point before uninstalling" />}
          />
          <SettingsRow
            label="Protect Windows and Microsoft components"
            sub={
              safety.protectSystem
                ? 'System files, drivers in use and Microsoft services are never offered for removal'
                : 'Not recommended – you will see items that Windows needs'
            }
            warn={!safety.protectSystem}
            control={<Switch checked={safety.protectSystem} onChange={(v) => set('safety', { protectSystem: v })} label="Protect Windows and Microsoft components" />}
          />
          <SettingsRow
            label="Pre-select leftovers"
            sub="What gets checked for you after a scan"
            control={
              <Segmented
                label="Pre-select leftovers"
                value={safety.preselect}
                onChange={(v) => set('safety', { preselect: v })}
                options={[
                  { value: 'high', label: 'High only' },
                  { value: 'high-medium', label: 'High & medium' },
                  { value: 'all', label: 'Everything' },
                ]}
              />
            }
          />
        </SettingsSection>

        <SettingsSection title="Leftover scan" note="Used by the leftover scan, which arrives in v0.2.">
          <SettingsRow
            label="Scan depth"
            sub="Deep also checks shared vendor folders and the whole registry"
            control={
              <Segmented
                label="Scan depth"
                value={scan.depth}
                onChange={(v) => set('scan', { depth: v })}
                options={[
                  { value: 'quick', label: 'Quick' },
                  { value: 'standard', label: 'Standard' },
                  { value: 'deep', label: 'Deep' },
                ]}
              />
            }
          />
          <SettingsRow label="Look in" sub="Places UWiper checks after an app is removed">
            <div className="flex flex-wrap gap-1.5">
              {LOOK_IN.map(([key, label]) => (
                <Chip key={key} on={scan.lookIn[key]} onToggle={() => set('scan', { lookIn: { ...scan.lookIn, [key]: !scan.lookIn[key] } })}>
                  {label}
                </Chip>
              ))}
            </div>
          </SettingsRow>
          <SettingsRow label="Never touch" sub="Folders and registry keys UWiper always skips">
            <ExclusionList value={scan.exclusions} onChange={(exclusions) => set('scan', { exclusions })} />
          </SettingsRow>
        </SettingsSection>

        <SettingsSection title="Appearance">
          <SettingsRow
            label="Accent color"
            control={
              <div role="group" aria-label="Accent color" className="flex gap-2.5">
                {ACCENTS.map(([color, name]) => (
                  <Swatch key={color} color={color} label={name} on={appearance.accent === color} onPick={() => set('appearance', { accent: color })} />
                ))}
              </div>
            }
          />
          <SettingsRow
            label="Window buttons"
            sub="Windows-style on the right, or macOS-style on the left"
            control={
              <Segmented
                label="Window buttons"
                value={appearance.windowButtons}
                onChange={(v) => set('appearance', { windowButtons: v })}
                options={[
                  { value: 'windows', label: 'Windows' },
                  { value: 'macos', label: 'macOS' },
                ]}
              />
            }
          />
          <SettingsRow
            label="Translucent window"
            sub={env?.material === 'mica' ? 'Lets the desktop show through (Mica)' : 'Needs Windows 11'}
            control={
              <Switch
                checked={appearance.translucent}
                onChange={(v) => set('appearance', { translucent: v })}
                label="Translucent window"
                disabled={env?.material !== 'mica'}
              />
            }
          />
        </SettingsSection>

        <SettingsSection title="Remove UWiper" danger>
          <SettingsRow
            label="UWiper is portable"
            sub={
              <>
                To remove it, delete its folder – that's all there is.
                {footprint.startWithWindows && ' Turn off “Start with Windows” first so the Run entry goes too.'}
              </>
            }
            mono={info?.home}
            warn={footprint.startWithWindows}
          />
        </SettingsSection>

        <p className="px-1 pt-1 text-small text-fg-4">
          UWiper {env?.version ?? ''} · {env?.elevated ? 'running as administrator' : 'running without administrator rights'}
        </p>
      </PageBody>
    </>
  );
}
