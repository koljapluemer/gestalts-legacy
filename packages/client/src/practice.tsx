import { useEffect, useMemo, useState } from 'react';
import { useRepo } from './react';
import { newId, stateKey, type Repo } from './repo';
import { fsrsStateValue, type Facet, type ReviewState } from './learning';
import { PracticeSession, type SessionEvent } from './session';

/** What a perspective logs its practice with; see `PracticeSession`. */
export type PracticeLog = {
  /** something to answer appeared (measures the next trial's `ms`) */
  shown(): void;
  /** one answer; writes the session note */
  trial(event?: SessionEvent): Promise<void>;
};

/**
 * The practice log of perspective `app`: `practice-session@1` notes, one per session, written by this page only.
 * Activity is any key or pointer press on the page while it's visible.
 */
export function usePracticeSession(app: string): PracticeLog {
  const repo = useRepo();
  const [session] = useState(() => new PracticeSession(app, newId));

  useEffect(() => {
    const flush = () => { for (const w of session.takeWrites()) void repo.putValue(w.key, w.value); };
    const act = () => session.activity(Date.now());
    const visibility = () => {
      if (document.visibilityState === 'hidden') { session.hide(Date.now()); flush(); } else act();
    };
    window.addEventListener('pointerdown', act, true);
    window.addEventListener('keydown', act, true);
    window.addEventListener('pagehide', flush);
    document.addEventListener('visibilitychange', visibility);
    return () => {
      window.removeEventListener('pointerdown', act, true);
      window.removeEventListener('keydown', act, true);
      window.removeEventListener('pagehide', flush);
      document.removeEventListener('visibilitychange', visibility);
      session.activity(Date.now());
      flush();
    };
  }, [repo, session]);

  return useMemo(() => ({
    shown: () => session.shown(Date.now()),
    trial: async (event?: SessionEvent) => {
      session.trial(Date.now(), event);
      await Promise.all(session.takeWrites().map((w) => repo.putValue(w.key, w.value)));
    },
  }), [repo, session]);
}

/**
 * Saves `next` as the FSRS state of `f` (under `prev`'s key if it has one, else `stateKey`) and, with a `log`,
 * records an `fsrs-review` trial. Returns `next` with its saved key.
 */
export async function saveReview(repo: Repo, f: Facet, prev: ReviewState | undefined, next: ReviewState, log?: PracticeLog): Promise<ReviewState> {
  const key = prev?.key ?? stateKey('fsrs-state', f.target, f.facet);
  const saved = await repo.putValue(key, fsrsStateValue(key, f, next));
  await log?.trial({ kind: 'fsrs-review', target: f.target, ...(f.facet ? { facet: f.facet } : {}), rating: next.lastRating });
  return { ...next, key: saved.key };
}
