import { Settings } from 'lucide-react';
import { RepoProvider, useOpenRepo } from '@gestalts/client';
import { GreekAlphabetPane } from '@gestalts/ui/panes';
import { ConnectorSettings } from '@gestalts/ui/perspectives';

/** Shipped words come from public/gestalt-data; the learner's progress stays in this browser (OPFS). */
const SOURCE = { kind: 'layered', staticBase: new URL('gestalt-data', document.baseURI).href, dir: 'demo' } as const;

export function App() {
  const { repo, error } = useOpenRepo(SOURCE);
  return (
    <div className="flex min-h-dvh flex-col bg-base-200">
      <header className="navbar min-h-0 bg-base-100 px-4 py-1 shadow-sm">
        <span className="flex-1 font-semibold">Ancient Greek letters</span>
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
        {repo && <RepoProvider repo={repo}><GreekAlphabetPane /></RepoProvider>}
      </main>
      <footer className="p-4 text-center text-xs text-base-content/50">Pronunciation generated with espeak-ng. Your progress is stored only in this browser.</footer>
    </div>
  );
}
