import { useState } from 'react';
import { api, errorText } from '../../lib/api';
import { formatBytes, plural } from '../../lib/format';
import type { CleanReport, CleanupItem } from '../../lib/types';
import { Button, Callout, Dialog, Icon, Note, Spinner } from '../../ui';

type Step = { name: 'confirm' } | { name: 'cleaning' } | { name: 'done'; report: CleanReport } | { name: 'failed'; error: string };

/** Confirms, runs and reports a cache cleanup. `onClose(true)` once something was cleaned. */
export function CleanDialog({ items, onClose }: { items: CleanupItem[]; onClose: (cleaned: boolean) => void }) {
  const [step, setStep] = useState<Step>({ name: 'confirm' });
  const bytes = items.reduce((s, i) => s + i.sizeBytes, 0);

  const clean = async () => {
    setStep({ name: 'cleaning' });
    try {
      setStep({ name: 'done', report: await api.cleanCaches(items.map((i) => i.id)) });
    } catch (e) {
      setStep({ name: 'failed', error: errorText(e) });
    }
  };

  if (step.name === 'confirm') {
    return (
      <Dialog
        label="Clean caches"
        onClose={() => onClose(false)}
        footer={
          <>
            <Button onClick={() => onClose(false)}>Cancel</Button>
            <Button kind="primary" icon="sparkle" onClick={clean}>
              Clean {formatBytes(bytes)}
            </Button>
          </>
        }
      >
        <h2 className="text-h2 font-bold">Clean {formatBytes(bytes)}?</h2>
        <p className="mt-2 text-body leading-normal text-fg-3">
          These are deleted for good – apps rebuild them as needed. Close your browsers first to clean their caches completely.
        </p>
        <ul className="mt-4 flex flex-col divide-y divide-line rounded-xl border border-line-2 bg-fill">
          {items.map((i) => (
            <li key={i.id} className="flex items-center gap-3 px-3.5 py-2">
              <span className="flex-1 truncate text-meta">{i.name}</span>
              <span className="text-meta tabular-nums text-fg-3">{formatBytes(i.sizeBytes)}</span>
            </li>
          ))}
        </ul>
      </Dialog>
    );
  }

  if (step.name === 'cleaning') {
    return (
      <Dialog label="Cleaning caches">
        <div className="flex items-center gap-3 py-2">
          <Spinner size={18} />
          <div>
            <div className="font-medium">Cleaning {plural(items.length, 'category', 'categories')}…</div>
            <div className="mt-0.5 text-small text-fg-3">Thousands of small files can take a moment.</div>
          </div>
        </div>
      </Dialog>
    );
  }

  if (step.name === 'failed') {
    return (
      <Dialog label="Cleaning caches" onClose={() => onClose(true)} footer={<Button onClick={() => onClose(true)}>Close</Button>}>
        <Callout message={step.error} />
      </Dialog>
    );
  }

  const { report } = step;
  return (
    <Dialog
      label="Caches cleaned"
      onClose={() => onClose(true)}
      footer={
        <Button kind="primary" onClick={() => onClose(true)}>
          Done
        </Button>
      }
    >
      <div className="flex flex-col items-center gap-2 py-2 text-center">
        <span className="grid size-14 place-items-center rounded-full bg-good/14 text-good-fg">
          <Icon name="check" size={28} stroke={2.4} />
        </span>
        <h2 className="text-h2 font-bold">{report.freedBytes > 0 ? `${formatBytes(report.freedBytes)} freed` : 'Nothing to free'}</h2>
        {report.skippedFiles + report.deniedFiles + report.failedFiles + report.skippedLinks + report.unreadableFolders + report.refused.length > 0 && (
          <Note>
            {[
              report.skippedFiles > 0 && `${plural(report.skippedFiles, 'file was', 'files were')} in use – close the apps that use them and clean again.`,
              report.deniedFiles > 0 && `Windows didn't let UWiper delete ${plural(report.deniedFiles, 'file', 'files')}.`,
              report.failedFiles > 0 && `${plural(report.failedFiles, 'file', 'files')} couldn't be deleted.`,
              report.skippedLinks > 0 && `${plural(report.skippedLinks, 'folder was', 'folders were')} left alone: ${report.skippedLinks > 1 ? 'they lead' : 'it leads'} through a link elsewhere.`,
              report.unreadableFolders > 0 && `${plural(report.unreadableFolders, 'folder', 'folders')} couldn't be opened.`,
              report.refused.length > 0 &&
                `${report.refused.join(', ')} ${report.refused.length > 1 ? 'hold' : 'holds'} your own files, so ${report.refused.length > 1 ? 'they were' : 'it was'} left alone – check where %TEMP% points.`,
            ]
              .filter(Boolean)
              .join(' ')}
          </Note>
        )}
      </div>
    </Dialog>
  );
}
