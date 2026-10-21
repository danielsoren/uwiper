import { useState } from 'react';
import { inTauri, pickFolder } from '../../lib/api';
import { Button, Field, Icon, IconButton } from '../../ui';

/** Folders and registry keys the leftover scan must never touch. */
export function ExclusionList({ value, onChange }: { value: string[]; onChange: (next: string[]) => void }) {
  const [draft, setDraft] = useState('');

  const add = (entry: string) => {
    const e = entry.trim();
    if (e && !value.some((v) => v.toLowerCase() === e.toLowerCase())) onChange([...value, e]);
    setDraft('');
  };

  return (
    <div className="flex flex-col gap-1.5">
      {value.map((entry) => (
        <div key={entry} className="flex min-h-9 items-center gap-2.5 rounded-lg bg-white/4 pl-3 pr-1.5">
          <Icon name={/^HK/i.test(entry) ? 'cube' : 'folder'} size={15} className="text-fg-4" />
          <span className="flex-1 select-text truncate font-mono text-small">{entry}</span>
          <IconButton icon="close" label={`Stop excluding ${entry}`} onClick={() => onChange(value.filter((v) => v !== entry))} />
        </div>
      ))}
      <div className="flex flex-wrap items-center gap-2">
        <Field
          mono
          icon="plus"
          label="Add an exclusion"
          placeholder="Folder or registry key, e.g. HKCU\Software\Vendor"
          value={draft}
          onChange={setDraft}
          onEnter={() => add(draft)}
          className="flex-[1_1_280px]"
        />
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
