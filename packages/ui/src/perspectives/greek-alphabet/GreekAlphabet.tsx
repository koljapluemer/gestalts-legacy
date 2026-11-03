import { useEffect, useMemo, useState } from 'react';
import { blob, fsrsStates, getScheduler, logger, Rating, saveReview, str, useBlobUrl, usePracticeSession, type Grade, type IndexedNote, type Query } from '@gestalts/client';
import { definePerspective } from '../define/definePerspective';
import { facetOf, isStruggling, nextCard, rate } from './schedule';

const glog = logger('greek');

type Props = {
  /** which pronunciation cards to practise */
  deckTag?: string;
};

const GRADES: { grade: Grade; label: string; className: string }[] = [
  { grade: Rating.Again, label: 'Again', className: 'btn-error' },
  { grade: Rating.Hard, label: 'Hard', className: 'btn-warning' },
  { grade: Rating.Good, label: 'Good', className: 'btn-success' },
  { grade: Rating.Easy, label: 'Easy', className: 'btn-info' },
];

/** Read a Greek word aloud, then check against the recorded pronunciation and rate yourself. */
export const GreekAlphabet = definePerspective<{ cards: Query; reviews: Query }, Props>({
  id: 'greek-alphabet',
  title: 'Greek alphabet',
  data: ({ deckTag = 'greek-alphabet' }) => ({
    cards: { gestalt: 'pronunciation-card@1', tagsAll: [deckTag], tagsNone: ['retired'] },
    reviews: { gestalt: 'fsrs-state@1' },
  }),
  Component: ({ data, repo }) => {
    const states = useMemo(() => fsrsStates(data.reviews), [data.reviews]);
    const log = usePracticeSession('greek-alphabet');
    const [current, setCurrent] = useState<{ id: string | null; last: string | null; picked: boolean }>({ id: null, last: null, picked: false });

    // first pick, once the cards are there
    const cardsLoaded = data.cards.length > 0;
    useEffect(() => {
      if (!cardsLoaded || current.picked) return;
      const c = nextCard(data.cards, states, null, new Date());
      setCurrent({ id: c?.id ?? null, last: null, picked: true });
      // eslint-disable-next-line react-hooks/exhaustive-deps -- only the first pick
    }, [cardsLoaded]);

    const card = data.cards.find((c) => c.id === current.id);

    const onRate = async (grade: Grade) => {
      if (!card) return;
      const now = new Date();
      const f = facetOf(card.id);
      const prev = states.get(f);
      glog.info('rate', { id: card.id, grade, prevDue: prev?.card.dueAt ?? null });
      const saved = await saveReview(repo, f, prev, rate(await getScheduler(), prev, grade, now), log);
      const updated = states.with(f, saved);
      const following = nextCard(data.cards, updated, card.id, now);
      glog.info('rated', { id: card.id, nextDue: saved.card.dueAt, next: following?.id ?? null });
      setCurrent({ id: following?.id ?? null, last: card.id, picked: true });
    };

    if (!data.cards.length) return <p className="p-8 text-center text-base-content/60">No pronunciation cards here yet.</p>;
    if (!current.picked) return <div className="flex justify-center p-8"><span className="loading loading-spinner" /></div>;
    if (!card) {
      return (
        <div className="flex flex-col items-center gap-3 p-8 text-center">
          <p className="text-lg font-semibold">All caught up.</p>
          <p className="text-base-content/60">Nothing due right now.</p>
          <button type="button" className="btn btn-sm" onClick={() => {
            const c = nextCard(data.cards, states, null, new Date());
            setCurrent({ id: c?.id ?? null, last: null, picked: true });
          }}>Check again</button>
        </div>
      );
    }
    return <PracticeCard key={card.key} card={card} struggling={isStruggling(states.get(facetOf(card.id)))} onShown={log.shown} onRate={onRate} />;
  },
});

function PracticeCard({ card, struggling, onShown, onRate }: { card: IndexedNote; struggling: boolean; onShown: () => void; onRate: (g: Grade) => Promise<void> }) {
  const [revealed, setRevealed] = useState(false);
  useEffect(onShown, [onShown]);
  const [busy, setBusy] = useState(false);
  const image = useBlobUrl(blob(card, 'image'));
  const audio = useBlobUrl(revealed ? blob(card, 'audio') : undefined);
  const term = str(card, 'title') ?? '';
  const lang = str(card, 'lang');

  return (
    <div className="card card-border mx-auto w-full max-w-md bg-base-100">
      <div className="card-body items-center gap-4 text-center">
        <p className="text-sm text-base-content/60">Try to pronounce this</p>
        <p lang={lang} className="text-5xl leading-tight break-words">{term}</p>
        {(struggling || revealed) && image && <img src={image} alt="" className="rounded-box max-h-56 object-contain" />}
        {!revealed ? (
          <button type="button" className="btn btn-primary" onClick={() => setRevealed(true)}>Show Pronunciation</button>
        ) : (
          <>
            {audio && <audio controls autoPlay src={audio} className="w-full" />}
            <div className="grid w-full grid-cols-4 gap-2">
              {GRADES.map((g) => (
                <button
                  key={g.grade}
                  type="button"
                  disabled={busy}
                  className={`btn btn-sm btn-soft ${g.className}`}
                  onClick={() => { setBusy(true); void onRate(g.grade).finally(() => setBusy(false)); }}
                >
                  {g.label}
                </button>
              ))}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
