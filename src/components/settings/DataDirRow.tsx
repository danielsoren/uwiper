import { useState } from 'react';
import { inTauri, pickFolder } from '../../lib/api';
import { formatBytes } from '../../lib/format';
import type { DataDir, DataDirKey } from '../../lib/types';
import { Button, Field, Pill } from '../../ui';
import { Locked, SettingsRow } from './SettingsSection';

const META: Record<DataDirKey, { label: string; sub: string }> = {
  quarantine: { label: 'Quarantine', sub: 'Removed files plus .reg backups of deleted registry keys' },
  snapshots: { label: 'Install snapshots', sub: 'What each tracked installer created' },
  logs: { label: 'Logs', sub: 'What UWiper did and when' },
  temp: { label: 'Temporary files', sub: 'Emptied every time UWiper closes' },
};

/** A data folder with its size, and an inline editor when custom locations are on. */
export function DataDirRow({ dir, editable, onSave }: { dir: DataDir; editable: boolean; onSave: (path: string | null) => void }) {
  const [draft, setDraft] = useState<string | null>(null);
  const meta = META[dir.key];

  const save = (value: string | null) => {
    const clean = value?.trim().replace(/[\\/]+$/, '');
    onSave(clean ? clean : null);
    setDraft(null);
  };
  const browse = async () => {
    const picked = await pickFolder(`Choose a folder for ${meta.label.toLowerCase()}`);
    if (picked) setDraft(picked);
  };

  return (
    <SettingsRow
      label={meta.label}
      sub={meta.sub}
      mono={dir.path}
      control={
        <div className="flex flex-wrap items-center justify-end gap-2">
          {!dir.inside && <Pill tone="warn">Outside UWiper folder</Pill>}
          <span className="text-small text-fg-3">{dir.exists ? formatBytes(dir.sizeBytes) : 'Not created yet'}</span>
          {!editable ? (
            <Locked>In UWiper folder</Locked>
          ) : (
            draft === null && (
              <Button small onClick={() => setDraft(dir.path)}>
                Change…
              </Button>
            )
          )}
        </div>
      }
    >
      {editable && draft !== null && (
        <div className="flex flex-wrap items-center gap-2">
          <Field
            mono
            icon="folder"
            label={`New location for ${meta.label}`}
            value={draft}
            onChange={setDraft}
            onEnter={() => save(draft)}
            onEscape={() => setDraft(null)}
            className="flex-[1_1_280px]"
          />
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
    </SettingsRow>
  );
}
