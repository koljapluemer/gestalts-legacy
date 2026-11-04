import { Blocks, Cable } from 'lucide-react';
import { distribution, plugins } from 'virtual:gestalts-plugins';
import { RepoProvider } from '@gestalts/client';
import { DebugLogButton } from '@gestalts/ui/debug';
import { PaneTabs, PluginSettings, usePluginRepo, type PluginEntry } from '@gestalts/ui/host';
import { ConnectorSettings, useConnectorState } from '@gestalts/ui/perspectives';

const REGISTRY = { distribution, plugins };

export function App() {
  const { repo, installed, error } = usePluginRepo('gestalter', REGISTRY);
  if (error) return <Failure message={error} />;
  if (!repo) return <div className="flex h-dvh items-center justify-center"><span className="loading loading-spinner" /></div>;
  return (
    <RepoProvider repo={repo}>
      <PaneTabs registry={REGISTRY} title="Gestalter" end={<>
        <DebugLogButton />
        {distribution.install_ui && <PluginsMenu mounted={installed} />}
        <ConnectorMenu />
      </>} />
    </RepoProvider>
  );
}

function PluginsMenu({ mounted }: { mounted: PluginEntry[] }) {
  return (
    <details className="dropdown dropdown-end">
      <summary className="btn btn-sm btn-ghost btn-square" title="Plugins"><Blocks className="size-4" /></summary>
      <div className="dropdown-content z-50 mt-2 rounded-box border border-base-300 bg-base-100 p-3 shadow">
        <PluginSettings registry={REGISTRY} mounted={mounted} />
      </div>
    </details>
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
