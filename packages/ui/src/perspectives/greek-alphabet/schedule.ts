import { DefaultScheduler, Rating, type Grade } from 'ts-fsrs';
import { asMap, field, relationships, type IndexedNote, type Json } from '@gestalts/client';

/**
 * Spaced repetition for pronunciation cards, stored as `fsrs-state@1` notes.
 * Ported from sam-learns-things' ancient-greek-alphabet, on ts-fsrs 6 (FSRS-7).
 */

export const MODEL = 'FSRS-7' as const;
export const DUE_PREFERENCE = 0.7;

const makeScheduler = () => DefaultScheduler({ version: MODEL });
type Scheduler = Awaited<ReturnType<typeof makeScheduler>>;
type SchedulerCard = Parameters<Scheduler['review']>[0]['card'];

let scheduler: Promise<Scheduler> | null = null;
export function getScheduler(): Promise<Scheduler> {
  scheduler ??= makeScheduler();
  return scheduler;
}

/** A card's learner state as stored: the ts-fsrs card with dates as ISO strings. */
export type ReviewState = {
  /** key of the fsrs-state note, if saved */
  key?: string;
  lastRating: Grade;
  card: { [k: string]: Json } & { dueAt: string };
};

export function stateFromNote(n: IndexedNote): { reviews: string; state: ReviewState } | null {
  const reviews = relationships(n).find((r) => r.label === 'reviews')?.target;
  const card = asMap(field(n, 'card'));
  const lastRating = field(n, 'last_rating');
  if (!reviews || !card || typeof card.dueAt !== 'string' || typeof lastRating !== 'number') return null;
  return { reviews, state: { key: n.key, lastRating: lastRating as Grade, card: card as ReviewState['card'] } };
}

/** `key`: the note's key, also its `id`; new states use `stateKey('fsrs-state', cardId)`. */
export function toNoteValue(key: string, cardId: string, s: ReviewState): Json {
  return {
    id: key,
    tags: ['fsrs-state'],
    relationships: [{ to: cardId, label: 'reviews' }],
    model: MODEL,
    last_rating: s.lastRating,
    card: s.card,
  };
}

/** Unseen, or last rated Hard or Again: show the image hint up front. */
export function isStruggling(s: ReviewState | undefined): boolean {
  return !s || s.lastRating <= Rating.Hard;
}

function toSchedulerCard(stored: ReviewState['card']): SchedulerCard {
  const lastReviewAt = stored.lastReviewAt;
  return { ...stored, dueAt: new Date(stored.dueAt), lastReviewAt: typeof lastReviewAt === 'string' ? new Date(lastReviewAt) : null } as unknown as SchedulerCard;
}

function toStoredCard(card: SchedulerCard): ReviewState['card'] {
  return JSON.parse(JSON.stringify(card)) as ReviewState['card'];
}

/** The state after rating; when the card stops or starts struggling it is due again right away. */
export function rate(s: Scheduler, prev: ReviewState | undefined, grade: Grade, now: Date): ReviewState {
  const card = prev ? toSchedulerCard(prev.card) : (s.newCard({ now }) as SchedulerCard);
  const reviewed = s.review({ card, grade, now }).card as SchedulerCard;
  const next: ReviewState = { key: prev?.key, lastRating: grade, card: toStoredCard(reviewed) };
  if (isStruggling(prev) !== isStruggling(next)) next.card.dueAt = now.toISOString();
  return next;
}

/**
 * Due cards with probability DUE_PREFERENCE, else unseen ones (falling back to the other group).
 * Never the previous card; seen cards that aren't due are skipped. `null`: nothing to practise now.
 */
export function nextCard<C extends { id: string }>(
  cards: C[],
  states: Map<string, ReviewState>,
  lastId: string | null,
  now: Date,
  random: () => number = Math.random,
): C | null {
  const candidates = cards.filter((c) => c.id !== lastId);
  const due = candidates.filter((c) => { const s = states.get(c.id); return s && new Date(s.card.dueAt) <= now; });
  const unseen = candidates.filter((c) => !states.has(c.id));
  const [preferred, fallback] = random() < DUE_PREFERENCE ? [due, unseen] : [unseen, due];
  const pool = preferred.length ? preferred : fallback;
  return pool.length ? pool[Math.floor(random() * pool.length)] : null;
}

export { Rating };
export type { Grade };
