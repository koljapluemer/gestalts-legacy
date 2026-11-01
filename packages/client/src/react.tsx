import { createContext, useContext, useEffect, useMemo, useState, useSyncExternalStore, type ReactNode } from 'react';
import { Repo, type RepoSource } from './repo';
import type { IndexedNote, Query } from './types';

const RepoContext = createContext<Repo | null>(null);

export function RepoProvider({ repo, children }: { repo: Repo; children: ReactNode }) {
  return <RepoContext value={repo}>{children}</RepoContext>;
}

export function useRepo(): Repo {
  const repo = useContext(RepoContext);
  if (!repo) throw new Error('useRepo needs a <RepoProvider>');
  return repo;
}

/** Re-renders on every change to the repo; returns its version. */
export function useRepoVersion(): number {
  const repo = useRepo();
  return useSyncExternalStore(repo.subscribe, () => repo.version);
}

/** The notes matching `q`, kept current. */
export function useQuery(q: Query): IndexedNote[] {
  const repo = useRepo();
  const version = useRepoVersion();
  const key = JSON.stringify(q);
  // eslint-disable-next-line react-hooks/exhaustive-deps -- `key` stands for `q`, `version` for the repo's content
  return useMemo(() => repo.query(JSON.parse(key) as Query), [repo, key, version]);
}

/** Resolves a perspective's data map, e.g. `{cards: {gestalt: 'flashcard@1'}}`, to notes. */
export function useData<K extends string>(spec: Record<K, Query>): Record<K, IndexedNote[]> {
  const repo = useRepo();
  const version = useRepoVersion();
  const key = JSON.stringify(spec);
  return useMemo(() => {
    const parsed = JSON.parse(key) as Record<K, Query>;
    return Object.fromEntries(Object.entries<Query>(parsed).map(([k, q]) => [k, repo.query(q)])) as Record<K, IndexedNote[]>;
    // eslint-disable-next-line react-hooks/exhaustive-deps -- see useQuery
  }, [repo, key, version]);
}

export function useNote(key: string | undefined): IndexedNote | undefined {
  const repo = useRepo();
  const version = useRepoVersion();
  // eslint-disable-next-line react-hooks/exhaustive-deps -- `version` stands for the repo's content
  return useMemo(() => (key ? repo.get(key) : undefined), [repo, key, version]);
}

/** An object URL for a blob, or null while loading / when missing. */
export function useBlobUrl(name: string | undefined): string | null {
  const repo = useRepo();
  const [url, setUrl] = useState<{ name: string; url: string | null } | null>(null);
  useEffect(() => {
    if (!name) return;
    let live = true;
    void repo.blobUrl(name).then((u) => { if (live) setUrl({ name, url: u }); });
    return () => { live = false; };
  }, [repo, name]);
  return url && url.name === name ? url.url : null;
}

export function useConnector() {
  const repo = useRepo();
  return useSyncExternalStore(repo.connector.subscribe, repo.connector.get);
}

const opened = new Map<string, Promise<Repo>>();

/** Opens a repo once per source (also across StrictMode's double effects); `error` says why it couldn't. */
export function useOpenRepo(source: RepoSource): { repo?: Repo; error?: string } {
  const [state, setState] = useState<{ repo?: Repo; error?: string }>({});
  const key = JSON.stringify(source);
  useEffect(() => {
    let live = true;
    let p = opened.get(key);
    if (!p) {
      p = Repo.open(JSON.parse(key) as RepoSource);
      opened.set(key, p);
    }
    p.then(
      (repo) => { if (live) setState({ repo }); },
      (e: unknown) => { opened.delete(key); if (live) setState({ error: e instanceof Error ? e.message : String(e) }); },
    );
    return () => { live = false; };
  }, [key]);
  return state;
}
