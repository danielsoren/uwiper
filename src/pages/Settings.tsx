import { useEffect, useState, type ReactNode } from 'react';
import { Icon } from '../components/Icon';
import { PageHeader } from '../components/PageHeader';
import { Button, Loading, Pill, Segmented, Switch } from '../components/ui';
import { api, inTauri, pickFolder } from '../lib/api';
import { formatBytes, plural } from '../lib/format';
import type { DataDir, DataDirKey, Settings as SettingsT } from '../lib/types';
import { useStore } from '../state/store';

const ACCENTS: [string, string][] = [
  ['#0071E3', 'Blue'],
  ['#8E44EC', 'Purple'],
  ['#D6336C', 'Pink'],
  ['#C2410C', 'Orange'],
];

const DIR_META: Record<DataDirKey, { label: string; sub: string }> = {
  quarantine: { label: 'Quarantine', sub: 'Removed files plus .reg backups of deleted registry keys' },
  snapshots: { label: 'Install snapshots', sub: 'What each tracked installer created' },
  logs: { label: 'Logs', sub: 'What UWiper did and when' },
  temp: { label: 'Temporary files', sub: 'Emptied every time UWiper closes' },
};

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

function Section({ title, note, children, danger }: { title: string; note?: string; children: ReactNode; danger?: boolean }) {
  return (
    <section>
      <h2 className="group-title">
        <span>{title}</span>
      </h2>
      {note && <p className="section-note">{note}</p>}
      <div className={`card settings-card${danger ? ' danger' : ''}`}>{children}</div>
    </section>
  );
}

function Row({
  label,
  sub,
  mono,
  control,
  children,
  warn,
}: {
  label: string;
  sub?: ReactNode;
  mono?: string;
  control?: ReactNode;
  children?: ReactNode;
  warn?: boolean;
}) {
  return (
    <div className="settings-row">
      <div className="settings-text">
        <div className="settings-label">{label}</div>
        {sub && <div className={`settings-sub${warn ? ' warn' : ''}`}>{sub}</div>}
        {mono && <div className="mono settings-mono">{mono}</div>}
      </div>
      {control && <div className="settings-control">{control}</div>}
      {children && <div className="settings-extra">{children}</div>}
    </div>
  );
}

export function Settings() {
  const { settings, updateSettings, storage, env, attempt } = useStore();
  useEffect(() => storage.load(), [storage.load]);

  if (!settings) {
    return (
      <>
        <PageHeader title="Settings" />
        <div className="page-body">
          <Loading label="Loading settings…" />
        </div>
      </>
    );
  }

  const set = <K extends keyof SettingsT>(section: K, patch: Partial<SettingsT[K]>) =>
    updateSettings((s) => ({ ...s, [section]: { ...s[section], ...patch } }));

  const info = storage.data;
  const outsideDirs = (info?.dirs ?? []).filter((d) => !d.inside);
  const outsideEntries = settings.footprint.startWithWindows ? 1 : 0;

  return (
    <>
      <PageHeader title="Settings" subtitle="Changes are saved right away" />
      <div className="page-body narrow">
        <div className="card pad home-card">
          <span className="stat-icon tone-accent large">
            <Icon name="folder" size={22} />
          </span>
          <div className="min-0 flex-1">
            <h2 className="item-name big">
              {outsideDirs.length ? `UWiper folder plus ${plural(outsideDirs.length, 'custom location', 'custom locations')}` : 'Everything UWiper has is in one folder'}
            </h2>
            <div className="mono path-line">{info?.home ?? env?.home ?? '…'}</div>
            <div className="item-sub">
              {info ? `${formatBytes(info.homeSizeBytes)} inside` : 'Measuring…'} ·{' '}
              {outsideEntries + outsideDirs.length === 0
                ? 'nothing anywhere else'
                : `${plural(outsideEntries + outsideDirs.length, 'thing', 'things')} elsewhere, all listed below`}
            </div>
          </div>
          {info && (
            <Button small icon="folder" onClick={() => attempt(() => api.openFolder(info.home))}>
              Open
            </Button>
          )}
        </div>

        <Section title="Storage" note="You decide where UWiper keeps its data. By default it never writes to AppData, ProgramData or the registry.">
          <Row
            label="Keep everything in the UWiper folder"
            sub={settings.storage.oneFolder ? 'Quarantine, snapshots, logs and temp files stay next to the app' : 'Off – pick a location for each one below'}
            control={<Switch checked={settings.storage.oneFolder} onChange={(v) => set('storage', { oneFolder: v })} label="Keep everything in the UWiper folder" />}
          />
          <Row
            label="Settings file"
            sub="Plain text, safe to back up or copy to another PC"
            mono={info?.settingsFile}
            control={
              <span className="locked">
                <Icon name="lock" size={13} stroke={2} /> Always next to UWiper
              </span>
            }
          />
          {(info?.dirs ?? []).map((dir) => (
            <DirRow
              key={dir.key}
              dir={dir}
              editable={!settings.storage.oneFolder}
              onSave={(path) => set('storage', { [dir.key]: path } as Partial<SettingsT['storage']>)}
            />
          ))}
          <Row
            label="Keep logs for"
            control={
              <Segmented<string>
                label="Keep logs for"
                value={String(settings.storage.logDays)}
                onChange={(v) => set('storage', { logDays: Number(v) })}
                options={[
                  { value: '7', label: '7 days' },
                  { value: '30', label: '30 days' },
                  { value: '90', label: '90 days' },
                ]}
              />
            }
          />
        </Section>

        <Section title="Outside the UWiper folder" note="Every place UWiper writes anywhere else. Nothing else is ever created.">
          <Row
            label="Start with Windows"
            sub="Off by default – UWiper doesn't need to run in the background"
            mono="HKCU\…\CurrentVersion\Run\UWiper"
            control={
              <Switch
                checked={settings.footprint.startWithWindows}
                onChange={(v) => set('footprint', { startWithWindows: v })}
                label="Start with Windows"
              />
            }
          />
          <Row label="WebView data" sub="The browser engine's profile, kept inside the UWiper folder" mono={info ? `${info.home}\\webview` : undefined} control={<Pill tone="good">Inside</Pill>} />
          <Row label="Start menu shortcut" sub="UWiper is portable and doesn't add one" control={<Pill>Not created</Pill>} />
          <Row label={'"Installed apps" entry'} sub="Not needed for a portable app – delete the folder to remove it" control={<Pill>Not created</Pill>} />
        </Section>

        <Section title="Safety" note="Used by uninstall and cleanup, which arrive in v0.2.">
          <Row
            label="Quarantine"
            sub={settings.safety.quarantine ? 'Keep everything UWiper removes so it can be put back' : 'Off – removed items are deleted for good right away'}
            warn={!settings.safety.quarantine}
            control={<Switch checked={settings.safety.quarantine} onChange={(v) => set('safety', { quarantine: v })} label="Quarantine" />}
          />
          <Row
            label="Keep items for"
            sub="After that, a batch is deleted for good"
            control={
              <Segmented<string>
                label="Keep items for"
                value={String(settings.safety.keepDays)}
                onChange={(v) => set('safety', { keepDays: Number(v) })}
                options={['7', '14', '30', '90'].map((d) => ({ value: d, label: `${d} days` }))}
              />
            }
          />
          <Row
            label="Restore point before uninstalling"
            sub="Windows System Restore, created automatically"
            control={<Switch checked={settings.safety.restorePoint} onChange={(v) => set('safety', { restorePoint: v })} label="Restore point before uninstalling" />}
          />
          <Row
            label="Protect Windows and Microsoft components"
            sub={settings.safety.protectSystem ? 'System files, drivers in use and Microsoft services are never offered for removal' : 'Not recommended – you will see items that Windows needs'}
            warn={!settings.safety.protectSystem}
            control={<Switch checked={settings.safety.protectSystem} onChange={(v) => set('safety', { protectSystem: v })} label="Protect Windows and Microsoft components" />}
          />
          <Row
            label="Pre-select leftovers"
            sub="What gets checked for you after a scan"
            control={
              <Segmented<SettingsT['safety']['preselect']>
                label="Pre-select leftovers"
                value={settings.safety.preselect}
                onChange={(v) => set('safety', { preselect: v })}
                options={[
                  { value: 'high', label: 'High only' },
                  { value: 'high-medium', label: 'High & medium' },
                  { value: 'all', label: 'Everything' },
                ]}
              />
            }
          />
        </Section>

        <Section title="Leftover scan" note="Used by the leftover scan, which arrives in v0.2.">
          <Row
            label="Scan depth"
            sub="Deep also checks shared vendor folders and the whole registry"
            control={
              <Segmented<SettingsT['scan']['depth']>
                label="Scan depth"
                value={settings.scan.depth}
                onChange={(v) => set('scan', { depth: v })}
                options={[
                  { value: 'quick', label: 'Quick' },
                  { value: 'standard', label: 'Standard' },
                  { value: 'deep', label: 'Deep' },
                ]}
              />
            }
          />
          <Row label="Look in" sub="Places UWiper checks after an app is removed">
            <div className="chips">
              {LOOK_IN.map(([key, label]) => {
                const on = settings.scan.lookIn[key];
                return (
                  <button
                    key={key}
                    type="button"
                    className={`chip${on ? ' on' : ''}`}
                    aria-pressed={on}
                    onClick={() => set('scan', { lookIn: { ...settings.scan.lookIn, [key]: !on } })}
                  >
                    {on && <Icon name="check" size={12} stroke={3} />}
                    {label}
                  </button>
                );
              })}
            </div>
          </Row>
          <Row label="Never touch" sub="Folders and registry keys UWiper always skips">
            <Exclusions value={settings.scan.exclusions} onChange={(exclusions) => set('scan', { exclusions })} />
          </Row>
        </Section>

        <Section title="Appearance">
          <Row
            label="Accent color"
            control={
              <div className="swatches" role="group" aria-label="Accent color">
                {ACCENTS.map(([color, name]) => (
                  <button
                    key={color}
                    type="button"
                    aria-label={name}
                    aria-pressed={settings.appearance.accent === color}
                    className={`swatch${settings.appearance.accent === color ? ' on' : ''}`}
                    style={{ background: color, color }}
                    onClick={() => set('appearance', { accent: color })}
                  />
                ))}
              </div>
            }
          />
          <Row
            label="Window buttons"
            sub="Windows-style on the right, or macOS-style on the left"
            control={
              <Segmented<SettingsT['appearance']['windowButtons']>
                label="Window buttons"
                value={settings.appearance.windowButtons}
                onChange={(v) => set('appearance', { windowButtons: v })}
                options={[
                  { value: 'windows', label: 'Windows' },
                  { value: 'macos', label: 'macOS' },
                ]}
              />
            }
          />
          <Row
            label="Translucent window"
            sub={env?.material === 'mica' ? 'Lets the desktop show through (Mica)' : 'Needs Windows 11'}
            control={
              <Switch
                checked={settings.appearance.translucent}
                onChange={(v) => set('appearance', { translucent: v })}
                label="Translucent window"
                disabled={env?.material !== 'mica'}
              />
            }
          />
        </Section>

        <Section title="Remove UWiper" danger>
          <Row
            label="UWiper is portable"
            sub={
              <>
                To remove it, delete its folder – that's all there is.
                {settings.footprint.startWithWindows && ' Turn off “Start with Windows” first so the Run entry goes too.'}
              </>
            }
            mono={info?.home}
            warn={settings.footprint.startWithWindows}
          />
        </Section>

        <p className="footer-line">
          UWiper {env?.version ?? ''} · {env?.elevated ? 'running as administrator' : 'running without administrator rights'}
        </p>
      </div>
    </>
  );
}

function DirRow({ dir, editable, onSave }: { dir: DataDir; editable: boolean; onSave: (path: string | null) => void }) {
  const [draft, setDraft] = useState<string | null>(null);
  const meta = DIR_META[dir.key];
  const save = (value: string | null) => {
    onSave(value && value.trim() ? value.trim().replace(/[\\/]+$/, '') : null);
    setDraft(null);
  };
  const browse = async () => {
    const picked = await pickFolder(`Choose a folder for ${meta.label.toLowerCase()}`);
    if (picked) setDraft(picked);
  };

  return (
    <Row
      label={meta.label}
      sub={meta.sub}
      mono={dir.path}
      control={
        <div className="row-gap tight">
          {!dir.inside && <Pill tone="warn">Outside UWiper folder</Pill>}
          <span className="muted-text">{dir.exists ? formatBytes(dir.sizeBytes) : 'Not created yet'}</span>
          {editable ? (
            draft === null && (
              <Button small onClick={() => setDraft(dir.path)}>
                Change…
              </Button>
            )
          ) : (
            <span className="locked">
              <Icon name="lock" size={13} stroke={2} /> In UWiper folder
            </span>
          )}
        </div>
      }
    >
      {editable && draft !== null && (
        <div className="path-editor">
          <label className="path-input">
            <Icon name="folder" size={15} />
            <input
              type="text"
              spellCheck={false}
              aria-label={`New location for ${meta.label}`}
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') save(draft);
                if (e.key === 'Escape') setDraft(null);
              }}
            />
          </label>
          {inTauri && (
            <Button small onClick={browse}>
              Browse…
            </Button>
          )}
          <Button small kind="ghost" onClick={() => save(null)}>
            Use default
          </Button>
          <Button small onClick={() => setDraft(null)}>
            Cancel
          </Button>
          <Button small kind="primary" onClick={() => save(draft)}>
            Use this folder
          </Button>
        </div>
      )}
    </Row>
  );
}

function Exclusions({ value, onChange }: { value: string[]; onChange: (v: string[]) => void }) {
  const [draft, setDraft] = useState('');
  const add = (entry: string) => {
    const e = entry.trim();
    if (e && !value.some((v) => v.toLowerCase() === e.toLowerCase())) onChange([...value, e]);
    setDraft('');
  };
  return (
    <div className="exclusions">
      {value.map((entry) => (
        <div key={entry} className="exclusion">
          <Icon name={/^HK/i.test(entry) ? 'cube' : 'folder'} size={15} className="muted" />
          <span className="mono flex-1 ellipsis">{entry}</span>
          <button type="button" className="icon-button" aria-label={`Stop excluding ${entry}`} onClick={() => onChange(value.filter((v) => v !== entry))}>
            <Icon name="close" size={14} stroke={2.2} />
          </button>
        </div>
      ))}
      <div className="path-editor">
        <label className="path-input">
          <Icon name="plus" size={15} />
          <input
            type="text"
            spellCheck={false}
            placeholder="Folder or registry key, e.g. HKCU\Software\Vendor"
            aria-label="Add an exclusion"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && add(draft)}
          />
        </label>
        {inTauri && (
          <Button
            small
            icon="folder"
            onClick={async () => {
              const picked = await pickFolder('Choose a folder UWiper should never touch');
              if (picked) add(picked);
            }}
          >
            Add folder
          </Button>
        )}
        <Button small kind="primary" onClick={() => add(draft)} disabled={!draft.trim()}>
          Add
        </Button>
      </div>
    </div>
  );
}
