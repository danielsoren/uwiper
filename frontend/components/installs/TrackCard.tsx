import { useEffect, useState } from 'react';
import { api, errorText, pickInstaller } from '../../lib/api';
import { cn } from '../../lib/cn';
import { formatBytes, plural } from '../../lib/format';
import type { Leftover, TraceDraft } from '../../lib/types';
import { useAppStore } from '../../store/useAppStore';
import { Button, Card, Empty, Icon, Note, Spinner } from '../../ui';
import { LeftoverReview } from '../leftovers/LeftoverReview';

type Step = 'intro' | 'snapshot' | 'installing' | 'comparing' | 'review';

const STEPS = ['Snapshot', 'Install', 'Review'];
const STEP_INDEX: Record<Step, number> = { intro: 0, snapshot: 0, installing: 1, comparing: 2, review: 2 };

function Steps({ step }: { step: Step }) {
  const at = STEP_INDEX[step];
  return (
    <ol className="flex flex-wrap items-center gap-2 text-small font-medium">
      {STEPS.map((label, i) => (
        <li key={label} className="flex items-center gap-2">
          {i > 0 && <span className="h-px w-5 bg-line-2" />}
          <span
            className={cn(
              'grid size-5 place-items-center rounded-full text-micro font-semibold',
              i < at ? 'bg-good/18 text-good' : i === at ? 'bg-accent text-white' : 'bg-white/8 text-fg-4',
            )}
          >
            {i < at ? <Icon name="check" size={12} stroke={3} /> : i + 1}
          </span>
          <span className={i === at ? 'text-fg' : 'text-fg-3'}>{label}</span>
        </li>
      ))}
    </ol>
  );
}

function Working({ text }: { text: string }) {
  return (
    <div className="flex items-center gap-3 py-6 text-body text-fg-2">
      <Spinner size={18} />
      {text}
    </div>
  );
}

/** A draft item in the shape the leftover review shows. */
const asLeftover = (i: TraceDraft['items'][number]): Leftover => ({
  id: i.id,
  kind: i.kind,
  label: i.label,
  reason: i.sure ? 'New' : 'Inside a folder that was already there – another app running at the same time may have made it',
  confidence: i.sure ? 'high' : 'medium',
  sizeBytes: i.sizeBytes,
  modified: null,
  needsAdmin: false,
  target: i.target,
});

/** Tracking an install, from the first snapshot to the saved trace. */
export function TrackCard({ onClose }: { onClose: () => void }) {
  const env = useAppStore((s) => s.env);
  const tracking = useAppStore((s) => s.res.tracking.data);
  const load = useAppStore((s) => s.load);
  const refresh = useAppStore((s) => s.refresh);
  const notify = useAppStore((s) => s.notify);
  const [step, setStep] = useState<Step>(tracking?.active ? 'installing' : 'intro');
  const [draft, setDraft] = useState<TraceDraft | null>(null);
  const [keep, setKeep] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);

  // While installing, keep an eye on the installer UWiper started.
  useEffect(() => {
    if (step !== 'installing') return;
    const timer = setInterval(() => load('tracking', true), 2000);
    return () => clearInterval(timer);
  }, [step, load]);

  // Tracking that was started earlier (before a restart, say) picks up here.
  useEffect(() => {
    if (tracking?.active && step === 'intro') setStep('installing');
  }, [tracking?.active, step]);

  const start = async (withInstaller: boolean) => {
    const installer = withInstaller ? await pickInstaller() : null;
    if (withInstaller && !installer) return;
    setStep('snapshot');
    try {
      await api.startTracking();
    } catch (e) {
      notify(errorText(e));
      setStep('intro');
      return;
    }
    if (installer) await launch(installer);
    load('tracking', true);
    setStep('installing');
  };

  const launch = async (path: string) => {
    try {
      await api.launchInstaller(path);
    } catch (e) {
      notify(errorText(e));
    }
    load('tracking', true);
  };

  const finish = async () => {
    setStep('comparing');
    try {
      const found = await api.finishTracking();
      setDraft(found);
      setKeep(new Set(found.items.filter((i) => i.sure).map((i) => i.id)));
      setStep('review');
    } catch (e) {
      notify(errorText(e));
      setStep('installing');
    }
  };

  const cancel = async () => {
    setBusy(true);
    try {
      await api.cancelTracking();
      refresh('tracking');
      onClose();
    } catch (e) {
      notify(errorText(e));
    }
    setBusy(false);
  };

  const save = async () => {
    if (!draft) return;
    setBusy(true);
    try {
      await api.saveTrace([...keep]);
      notify(`Saved what «${draft.name}» added`);
      refresh('traces', 'tracking', 'apps');
      onClose();
    } catch (e) {
      notify(errorText(e));
    }
    setBusy(false);
  };

  const kept = draft?.items.filter((i) => keep.has(i.id)) ?? [];
  const keptBytes = kept.reduce((s, i) => s + (i.sizeBytes ?? 0), 0);
  const since = tracking?.startedAt ? new Date(tracking.startedAt * 1000).toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' }) : null;

  return (
    <Card className="flex flex-col gap-4 px-[22px] py-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h2 className="text-title font-semibold">Track an install</h2>
        <Steps step={step} />
      </div>

      {step === 'intro' && (
        <>
          <p className="max-w-[640px] text-body leading-normal text-fg-2">
            UWiper takes a snapshot now, you install the app, and UWiper looks at what's new. Close other apps first – anything they
            add in the meantime could look like the installer's.
          </p>
          {!env?.elevated && <Note>Scheduled tasks are only seen with administrator rights – the rest is tracked either way.</Note>}
          <div className="flex flex-wrap gap-2.5">
            <Button kind="primary" icon="install" onClick={() => start(true)}>
              Choose installer…
            </Button>
            <Button onClick={() => start(false)}>I'll run it myself</Button>
            <Button kind="ghost" onClick={onClose}>
              Cancel
            </Button>
          </div>
        </>
      )}

      {step === 'snapshot' && <Working text="Taking a snapshot of your PC…" />}

      {step === 'installing' && (
        <>
          <div className="flex items-start gap-3">
            <span className={cn('mt-0.5 grid size-8 flex-none place-items-center rounded-full', tracking?.installerRunning ? 'bg-accent/20 text-accent-soft' : 'bg-white/8 text-fg-3')}>
              {tracking?.installerRunning ? <Spinner size={15} /> : <Icon name="install" size={16} />}
            </span>
            <div>
              <div className="text-label font-semibold">
                {tracking?.installer
                  ? tracking.installerRunning
                    ? `«${tracking.installer}» is running`
                    : `«${tracking.installer}» has closed`
                  : 'Run the installer now'}
              </div>
              <p className="mt-1 max-w-[640px] text-small leading-normal text-fg-3">
                {tracking?.installerRunning
                  ? 'Go through it the way you normally would. If it opens the app at the end, that first start is part of the trace too.'
                  : 'When the installer has finished – and the app has opened once, if you like – click Done.'}
                {since && ` Tracking since ${since}; you can close UWiper or restart Windows in between.`}
              </p>
            </div>
          </div>
          <div className="flex flex-wrap gap-2.5">
            <Button kind="primary" icon="check" onClick={finish}>
              Done installing
            </Button>
            {!tracking?.installer && (
              <Button
                icon="install"
                onClick={async () => {
                  const path = await pickInstaller();
                  if (path) await launch(path);
                }}
              >
                Choose installer…
              </Button>
            )}
            <Button kind="ghost" busy={busy} onClick={cancel}>
              Stop tracking
            </Button>
          </div>
        </>
      )}

      {step === 'comparing' && <Working text="Looking for what the installer added…" />}

      {step === 'review' && draft && (
        <>
          <div>
            <div className="text-label font-semibold">What «{draft.name}» added</div>
            <div className="mt-0.5 text-small text-fg-3">
              {[draft.publisher, draft.version && `version ${draft.version}`].filter(Boolean).join(' · ') || 'No app registered itself – the trace is named after the installer'}
            </div>
          </div>
          {draft.updated && <Note>«{draft.name}» was already installed, so this was an update: only what it added is here.</Note>}
          {draft.alsoInstalled.length > 0 && (
            <Note>
              Also installed: {draft.alsoInstalled.map((n) => `«${n}»`).join(', ')}. What belongs to {draft.alsoInstalled.length > 1 ? 'them' : 'it'} stays
              out of this trace – other apps may need {draft.alsoInstalled.length > 1 ? 'them' : 'it'}.
            </Note>
          )}
          {!draft.tasksChecked && <Note>Scheduled tasks weren't checked – UWiper needs administrator rights to see them.</Note>}
          {draft.items.length === 0 ? (
            <Empty icon="check" title="Nothing new">
              UWiper didn't find anything the installer added. If it's still running, go back and click Done once it has finished.
            </Empty>
          ) : (
            <>
              <p className="text-small leading-normal text-fg-3">
                Untick anything that isn't the app's. <strong className="font-semibold text-fg-2">Medium</strong> items appeared inside folders
                that were already there, so they're left out unless you tick them.
              </p>
              <LeftoverReview items={draft.items.map(asLeftover)} selected={keep} onChange={setKeep} canRemove={() => true} appTitle="Its entry in installed apps" />
            </>
          )}
          <div className="flex flex-wrap items-center justify-end gap-2.5">
            {kept.length > 0 && (
              <span className="mr-auto text-small text-fg-3">
                {plural(kept.length, 'item', 'items')} kept{keptBytes > 0 && ` · ${formatBytes(keptBytes)}`}
              </span>
            )}
            {draft.items.length === 0 && <Button onClick={() => setStep('installing')}>Back</Button>}
            <Button kind="ghost" busy={busy && kept.length === 0} onClick={cancel}>
              Discard
            </Button>
            <Button kind="primary" icon="check" disabled={kept.length === 0} busy={busy && kept.length > 0} onClick={save}>
              Save trace
            </Button>
          </div>
        </>
      )}
    </Card>
  );
}
