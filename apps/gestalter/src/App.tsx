import { useSyncExternalStore } from 'react';
import { Cable } from 'lucide-react';
import { RepoProvider, useOpenRepo } from '@gestalts/client';
import { panes } from '@gestalts/ui/panes';
import { ConnectorSettings, useConnectorState } from '@gestalts/ui/perspectives';

const SOURCE = { kind: 'opfs', dir: 'gestalter' } as const;

export function App() {
  const { repo, error } = useOpenRepo(SOURCE);
  if (error) return <Failure message={error} />;
  if (!repo) return <div className="flex h-dvh items-center justify-center"><span className="loading loading-spinner" /></div>;
  return <RepoProvider repo={repo}><Shell /></RepoProvider>;
}

function Shell() {
  const current = usePaneId();
  const entry = panes.find((p) => p.id === current) ?? panes[0];
  return (
    <div className="flex h-dvh flex-col bg-base-200">
      <nav className="navbar min-h-0 gap-2 border-b border-base-300 bg-base-100 px-3 py-1">
        <span className="font-semibold">Gestalter</span>
        <div className="join flex-1 overflow-x-auto">
          {panes.map((p) => (
            <button key={p.id} type="button" className={`btn btn-sm join-item ${p.id === entry.id ? 'btn-active' : ''}`} onClick={() => setPaneId(p.id)}>
              {p.label}
            </button>
          ))}
        </div>
        <ConnectorMenu />
      </nav>
      <main className="min-h-0 flex-1"><entry.Pane key={entry.id} /></main>
    </div>
  );
}

function ConnectorMenu() {
  const state = useConnectorState();
  const color = { off: '', ok: 'text-success', syncing: 'text-info', error: 'text-error' }[state];
  return (
    <details className="dropdown dropdown-end">
      <summary className={`btn btn-sm btn-ghost btn-square ${color}`} title={`Connector: ${state}`}>
        <Cable className="size-4" />
      </summary>
      <div className="dropdown-content z-50 mt-2 rounded-box border border-base-300 bg-base-100 p-3 shadow">
        <ConnectorSettings />
      </div>
    </details>
  );
}

function Failure({ message }: { message: string }) {
  return (
    <div className="flex h-dvh items-center justify-center p-4">
      <div role="alert" className="alert alert-error max-w-lg">Could not open local storage: {message}</div>
    </div>
  );
}

// the selected pane lives in the URL (?pane=), so reloads and links keep it
const paneListeners = new Set<() => void>();
function usePaneId(): string | null {
  return useSyncExternalStore(
    (l) => { paneListeners.add(l); window.addEventListener('popstate', l); return () => { paneListeners.delete(l); window.removeEventListener('popstate', l); }; },
    () => new URLSearchParams(location.search).get('pane'),
  );
}
function setPaneId(id: string) {
  // a fresh query string: other params belong to the previous pane
  history.pushState(null, '', `${location.pathname}?pane=${encodeURIComponent(id)}`);
  paneListeners.forEach((l) => l());
}
