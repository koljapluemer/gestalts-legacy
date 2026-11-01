import { useCallback, useSyncExternalStore } from 'react';

/** A comma-separated list in the URL's query string, so a reload keeps it. */
export function useUrlList(name: string): [string[], (next: string[]) => void] {
  const raw = useSyncExternalStore(subscribe, () => new URLSearchParams(location.search).get(name) ?? '');
  const set = useCallback((next: string[]) => {
    const params = new URLSearchParams(location.search);
    if (next.length) params.set(name, next.join(','));
    else params.delete(name);
    const qs = params.toString();
    history.replaceState(history.state, '', `${location.pathname}${qs ? `?${qs}` : ''}${location.hash}`);
    listeners.forEach((l) => l());
  }, [name]);
  return [raw ? raw.split(',') : [], set];
}

const listeners = new Set<() => void>();
function subscribe(l: () => void) {
  listeners.add(l);
  window.addEventListener('popstate', l);
  return () => { listeners.delete(l); window.removeEventListener('popstate', l); };
}
