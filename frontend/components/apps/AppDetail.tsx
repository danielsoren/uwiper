import { useState, type ReactNode } from 'react';
import { api, errorText } from '../../lib/api';
import { cn } from '../../lib/cn';
import { formatBytes, formatDate } from '../../lib/format';
import type { InstalledApp } from '../../lib/types';
import { useAppStore } from '../../store/useAppStore';
import { Button, Note, Spinner } from '../../ui';
import { AppTile } from './AppTile';

export type Measure = number | 'loading' | 'error';

function Fact({ label, children, wide, mono }: { label: string; children: ReactNode; wide?: boolean; mono?: boolean }) {
  return (
    <div className={cn(wide && 'col-span-full')}>
      <dt className="text-micro text-fg-4">{label}</dt>
      <dd className={cn('mt-0.5', mono ? 'select-text break-all font-mono text-small text-fg-path' : 'text-body font-medium')}>{children}</dd>
    </div>
  );
}

export function AppDetail({ app, measure }: { app: InstalledApp; measure: Measure | undefined }) {
  const attempt = useAppStore((s) => s.attempt);
  const notify = useAppStore((s) => s.notify);
  const [copied, setCopied] = useState(false);
  const command = app.quietUninstallString ?? app.uninstallString;

  const copy = () => {
    if (!command) return;
    navigator.clipboard.writeText(command).then(
      () => {
        setCopied(true);
        setTimeout(() => setCopied(false), 1500);
      },
      (e) => notify(errorText(e)),
    );
  };

  return (
    <div className="flex flex-col gap-[18px] px-[26px] pb-[26px] pt-5">
      <div className="flex items-center gap-3.5">
        <AppTile name={app.name} large />
        <div className="min-w-0">
          <h2 className="select-text text-h2 font-bold tracking-[-0.01em]">{app.name}</h2>
          <div className="mt-[3px] text-meta text-fg-3">
            {app.publisher ?? 'Unknown publisher'}
            {app.version && ` · version ${app.version}`}
          </div>
        </div>
      </div>

      <dl className="grid grid-cols-2 gap-x-4 gap-y-3 rounded-xl border border-white/7 bg-white/4 px-4 py-3.5">
        <Fact label="Size on disk">
          {measure === 'loading' ? (
            <span className="inline-flex items-center gap-1.5 text-fg-3">
              <Spinner size={13} /> Measuring…
            </span>
          ) : typeof measure === 'number' ? (
            formatBytes(measure)
          ) : (
            <>
              {formatBytes(app.sizeBytes)} <span className="text-small font-normal text-fg-3">reported</span>
            </>
          )}
        </Fact>
        <Fact label="Installed">{formatDate(app.installDate)}</Fact>
        <Fact label="Installer">{app.source === 'msi' ? 'Windows Installer (MSI)' : 'Classic installer'}</Fact>
        <Fact label="Installed for">
          {app.scope === 'user' ? 'Current user' : 'All users'}
          {app.arch && ` · ${app.arch}`}
        </Fact>
        <Fact label="Location" wide mono>
          {app.installLocation ?? 'Not reported by the installer'}
        </Fact>
        <Fact label="Registry entry" wide mono>
          {app.registryKey}
        </Fact>
        {command && (
          <Fact label="Uninstall command" wide mono>
            {command}
          </Fact>
        )}
      </dl>

      <div className="flex flex-wrap items-center gap-2.5">
        <Button kind="danger" icon="trash" disabled title="Arrives in v0.2">
          Uninstall completely
        </Button>
        {app.installLocation && (
          <Button icon="folder" onClick={() => attempt(() => api.openFolder(app.installLocation!))}>
            Open folder
          </Button>
        )}
        {command && (
          <Button kind="ghost" onClick={copy}>
            {copied ? 'Copied' : 'Copy uninstall command'}
          </Button>
        )}
      </div>

      <Note>
        <strong>Coming in v0.2:</strong> one-click uninstall that runs the app's own uninstaller, then finds everything it
        left behind – files, registry keys, services, tasks, shortcuts – and moves them to quarantine.
      </Note>
    </div>
  );
}
