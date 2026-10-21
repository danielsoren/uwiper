import { useEffect, useState } from 'react';
import { Icon } from '../components/Icon';
import { PageHeader } from '../components/PageHeader';
import { Button, Card, ErrorBox, Loading, Note, Spinner } from '../components/ui';
import { api } from '../lib/api';
import { formatBytes, plural } from '../lib/format';
import type { CleanupGroup } from '../lib/types';
import { useStore } from '../state/store';
import { GROUP_META } from './Overview';

const ORDER: CleanupGroup[] = ['windows', 'browsers', 'apps', 'developer'];

export function Cleanup() {
  const { cleanup, attempt } = useStore();
  const [open, setOpen] = useState<Record<string, boolean>>({});

  useEffect(() => cleanup.load(), [cleanup.load]);

  const items = cleanup.data ?? [];
  const total = items.reduce((s, i) => s + i.sizeBytes, 0);
  const safe = items.filter((i) => i.defaultOn).reduce((s, i) => s + i.sizeBytes, 0);
  const groups = ORDER.map((g) => ({ group: g, items: items.filter((i) => i.group === g).sort((a, b) => b.sizeBytes - a.sizeBytes) })).filter(
    (g) => g.items.length > 0,
  );

  return (
    <>
      <PageHeader title="Cleanup" subtitle="Caches, temporary files and logs that Windows and your apps rebuild on their own">
        <Button icon="refresh" onClick={() => cleanup.load(true)} disabled={cleanup.loading}>
          Scan again
        </Button>
        <Button kind="primary" icon="sparkle" disabled title="Arrives in v0.2">
          Clean {formatBytes(safe)}
        </Button>
      </PageHeader>
      <div className="page-body">
        {cleanup.error && <ErrorBox message={cleanup.error} onRetry={() => cleanup.load(true)} />}
        {!cleanup.data && cleanup.loading && <Loading label="Measuring caches – big ones can take a moment…" />}

        {cleanup.data && (
          <>
            <Card className="hero">
              <div className="hero-main">
                <h2 className="eyebrow">Safe to clean</h2>
                <div className="hero-value">
                  {formatBytes(safe).split(' ')[0]} <span>{formatBytes(safe).split(' ')[1]}</span>
                </div>
                <p className="hero-text">
                  of {formatBytes(total)} found. Developer caches and anything you might want to keep stay off by default.
                </p>
              </div>
              <div className="hero-side">
                <div className="stack-bar" role="img" aria-label="Found, by group">
                  {groups.map((g) => (
                    <span
                      key={g.group}
                      style={{ flexGrow: g.items.reduce((s, i) => s + i.sizeBytes, 0) || 1, background: GROUP_META[g.group].color }}
                    />
                  ))}
                </div>
                <div className="legend legend-inline">
                  {groups.map((g) => (
                    <span key={g.group} className="legend-chip">
                      <span className="legend-dot" style={{ background: GROUP_META[g.group].color }} />
                      {GROUP_META[g.group].label}
                      <span className="muted-text">{formatBytes(g.items.reduce((s, i) => s + i.sizeBytes, 0))}</span>
                    </span>
                  ))}
                </div>
              </div>
            </Card>

            <Note>
              This version measures only. Cleaning arrives in v0.2 – caches will be deleted for good since apps rebuild
              them, and cookies, passwords, history and offline downloads are never touched.
            </Note>

            {groups.map((g) => (
              <section key={g.group} className="card list-card">
                <div className="list-head">
                  <span className="item-icon" style={{ color: GROUP_META[g.group].color }}>
                    <Icon name={GROUP_META[g.group].icon} />
                  </span>
                  <h2>{GROUP_META[g.group].label}</h2>
                  <span className="muted-text">{formatBytes(g.items.reduce((s, i) => s + i.sizeBytes, 0))}</span>
                  {cleanup.loading && <Spinner size={14} />}
                </div>
                {g.items.map((i) => (
                  <div key={i.id} className="cleanup-item">
                    <button
                      type="button"
                      className="item-row clickable"
                      aria-expanded={!!open[i.id]}
                      onClick={() => setOpen((o) => ({ ...o, [i.id]: !o[i.id] }))}
                    >
                      <span className="item-text">
                        <span className="item-name">{i.name}</span>
                        <span className="item-sub">
                          {i.description} · {plural(i.fileCount, 'file', 'files')}
                        </span>
                      </span>
                      {!i.defaultOn && <span className="app-badge">Optional</span>}
                      <span className="item-size">{formatBytes(i.sizeBytes)}</span>
                      <Icon name="chevron" size={14} className={`muted chevron${open[i.id] ? ' open' : ''}`} />
                    </button>
                    {open[i.id] && (
                      <div className="paths">
                        {i.paths.map((p) => (
                          <button type="button" key={p} className="path-link mono" onClick={() => attempt(() => api.revealPath(p))}>
                            {p}
                          </button>
                        ))}
                      </div>
                    )}
                  </div>
                ))}
              </section>
            ))}
          </>
        )}
      </div>
    </>
  );
}
