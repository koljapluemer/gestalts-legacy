import { useEffect } from 'react';
import { Settings } from 'lucide-react';
import { logger, RepoProvider, useOpenRepo } from '@gestalts/client';
import { DebugLogButton, ErrorBoundary } from '@gestalts/ui/debug';
import { GreekAlphabetPane } from '@gestalts/ui/panes';
import { ConnectorSettings } from '@gestalts/ui/perspectives';

/**
 * Shipped words come from public/gestalt-data. A word is copied into this browser (OPFS) with the
 * learner's progress once it is first rated, so the learner's data stands on its own.
 */
const log = logger('demo');

const SOURCE = { kind: 'layered', staticBase: new URL('gestalt-data', document.baseURI).href, dir: 'demo' } as const;

export function App() {
  const { repo, error } = useOpenRepo(SOURCE);
  useEffect(() => {
    const r = repo?.upstreamReport();
    if (r && (r.copied.length || r.updated.length || r.diverged.length)) log.info('deck update', { copied: r.copied, updated: r.updated, diverged: r.diverged });
  }, [repo]);
  return (
    <div className="flex min-h-dvh flex-col bg-base-200">
      <header className="navbar min-h-0 bg-base-100 px-4 py-1 shadow-sm">
        <span className="flex-1 font-semibold">Ancient Greek letters</span>
        <DebugLogButton />
        {repo && (
          <details className="dropdown dropdown-end">
            <summary className="btn btn-sm btn-ghost btn-square" title="Sync settings"><Settings className="size-4" /></summary>
            <div className="dropdown-content z-50 mt-2 rounded-box border border-base-300 bg-base-100 p-3 shadow">
              <RepoProvider repo={repo}><ConnectorSettings /></RepoProvider>
            </div>
          </details>
        )}
      </header>
      <main className="flex-1">
        {error && <div role="alert" className="alert alert-error m-4">Could not load: {error}</div>}
        {!error && !repo && <div className="flex justify-center p-8"><span className="loading loading-spinner" /></div>}
        {repo && <RepoProvider repo={repo}><ErrorBoundary name="Greek alphabet"><GreekAlphabetPane /></ErrorBoundary></RepoProvider>}
      </main>
      <footer className="p-4 text-center text-xs text-base-content/50">Pronunciation generated with espeak-ng. Your progress is stored only in this browser.</footer>
    </div>
  );
}
