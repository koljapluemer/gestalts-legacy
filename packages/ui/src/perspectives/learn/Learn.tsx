import { useEffect, useMemo, useState } from 'react';
import { FileText } from 'lucide-react';
import {
  fsrsStates, getScheduler, hashing, learnableCards, pickNext, Rating, review, saveReview, slotOf, until, usePracticeSession,
  type FsrsStates, type Grade, type LearnCard, type Query,
} from '@gestalts/client';
import { Markdown } from '../../primitives/Markdown';
import { definePerspective } from '../define/definePerspective';

type Props = {
  /** open a card's note (its host, or the card note); without it there's no button for it */
  onOpen?: (key: string) => void;
};

const GRADES: { grade: Grade; label: string }[] = [
  { grade: Rating.Again, label: 'Again' },
  { grade: Rating.Hard, label: 'Hard' },
  { grade: Rating.Good, label: 'Good' },
  { grade: Rating.Easy, label: 'Easy' },
];

/** Keys typed into a field or over a dialog (the note editor) aren't for the card. */
function forCard(e: KeyboardEvent): boolean {
  const t = e.target as HTMLElement | null;
  const typing = t && (t.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT', 'BUTTON'].includes(t.tagName));
  return !typing && !e.metaKey && !e.ctrlKey && !e.altKey && !document.querySelector('dialog[open]');
}

/**
 * Spaced repetition of every flashcard (as dsl's Learn tab): a random due card, else a random new one. Its front,
 * then front and back with the four ratings.
 */
export const Learn = definePerspective<{ hosts: Query; flashcards: Query; reviews: Query }, Props>({
  id: 'learn',
  title: 'Learn',
  data: {
    hosts: { gestalt: 'cards@1', tagsNone: ['retired'] },
    flashcards: { gestalt: 'flashcard@1', tagsNone: ['retired'] },
    reviews: { gestalt: 'fsrs-state@1' },
  },
  Component: ({ data, repo, onOpen }) => {
    const cards = useMemo(() => learnableCards(data.hosts, data.flashcards, hashing), [data.hosts, data.flashcards]);
    const stored = useMemo(() => fsrsStates(data.reviews), [data.reviews]);
    // reviews saved here, until the query has them: the next pick mustn't wait for it
    const [saved, setSaved] = useState<{ base: FsrsStates; states: FsrsStates } | null>(null);
    const states = saved?.base === stored ? saved.states : stored;
    const log = usePracticeSession('learn');
    const [current, setCurrent] = useState<{ slot: string | null; last: string | null; at: number }>({ slot: null, last: null, at: Date.now() });

    const pick = useMemo(() => pickNext(cards, states, current.last, new Date(current.at)), [cards, states, current.last, current.at]);
    // the card shown stays until it's rated, even if the picks around it change (a sync, an edit)
    const shown = cards.find((c) => slotOf(c.facet) === current.slot) ?? null;
    const card = shown ?? pick.card;
    const slot = card ? slotOf(card.facet) : null;
    useEffect(() => {
      if (slot && slot !== current.slot) setCurrent((c) => ({ ...c, slot }));
    }, [slot, current.slot]);

    const onRate = async (c: LearnCard, grade: Grade) => {
      const at = new Date();
      const prev = states.get(c.facet);
      const next = await saveReview(repo, c.facet, prev, review(await getScheduler(), prev, grade, at), log);
      setSaved({ base: stored, states: states.with(c.facet, next) });
      setCurrent({ slot: null, last: slotOf(c.facet), at: Date.now() });
    };

    const counts = (
      <span className="ml-auto font-mono text-xs text-base-content/50" title="due · new · all cards">
        {pick.counts.due} · {pick.counts.new} · {pick.counts.total}
      </span>
    );
    return (
      <div className="mx-auto flex w-full max-w-2xl flex-col gap-4">
        <div className="flex min-h-8 items-center gap-1">
          {card && onOpen && (
            <button type="button" className="btn btn-ghost btn-sm btn-square" title="Open its note" onClick={() => onOpen(card.noteKey)}>
              <FileText className="size-4" />
            </button>
          )}
          {counts}
        </div>
        {card ? (
          <Review key={slot} card={card} onShown={log.shown} onOpen={onOpen} onRate={(g) => onRate(card, g)} />
        ) : (
          <div className="flex flex-col items-center gap-3 p-8 text-center text-base-content/60">
            <p>{!cards.length ? 'No flashcards.' : pick.nextDue ? `Nothing due. Next in ${until(pick.nextDue)}.` : 'Nothing due.'}</p>
            {cards.length > 0 && (
              <button type="button" className="btn btn-sm" onClick={() => setCurrent({ slot: null, last: null, at: Date.now() })}>Check again</button>
            )}
          </div>
        )}
      </div>
    );
  },
});

function Review({ card, onShown, onOpen, onRate }: { card: LearnCard; onShown: () => void; onOpen?: (key: string) => void; onRate: (g: Grade) => Promise<void> }) {
  const [revealed, setRevealed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(onShown, [onShown]);

  const rate = (g: Grade) => {
    if (busy) return;
    setBusy(true);
    setError(null);
    // stays busy on success: the next card replaces this one
    onRate(g).catch((e: unknown) => { setBusy(false); setError(e instanceof Error ? e.message : String(e)); });
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!forCard(e)) return;
      if (!revealed && e.key === ' ') { e.preventDefault(); setRevealed(true); }
      const g = GRADES[Number(e.key) - 1];
      if (revealed && g) { e.preventDefault(); rate(g.grade); }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  return (
    <>
      <div className="md-card rounded-box border border-base-300 bg-base-100 p-4">
        <Markdown text={card.front} onOpen={onOpen} />
        {revealed && <><hr className="my-3 border-base-300" /><Markdown text={card.back} onOpen={onOpen} /></>}
      </div>
      {revealed ? (
        <div className="grid grid-cols-4 gap-2">
          {GRADES.map((g, i) => (
            <button key={g.grade} type="button" className="btn" title={String(i + 1)} disabled={busy} onClick={() => rate(g.grade)}>{g.label}</button>
          ))}
        </div>
      ) : (
        <button type="button" className="btn btn-primary btn-block" title="Space" onClick={() => setRevealed(true)}>Reveal</button>
      )}
      {error && <div role="alert" className="alert alert-error text-sm">Couldn't save the review: {error}</div>}
    </>
  );
}
