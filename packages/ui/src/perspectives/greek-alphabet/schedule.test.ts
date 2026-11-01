import { describe, expect, it } from 'vitest';
import { getScheduler, isStruggling, nextCard, rate, Rating, type ReviewState } from './schedule';

const now = new Date('2026-11-01T12:00:00.000Z');
const cards = [{ id: 'a' }, { id: 'b' }, { id: 'c' }];

describe('rate', () => {
  it('creates and advances an FSRS card', async () => {
    const s = await getScheduler();
    const first = rate(s, undefined, Rating.Good, now);
    expect(first.lastRating).toBe(Rating.Good);
    expect(first.card.reps).toBe(1);
    expect(typeof first.card.dueAt).toBe('string');
    // was struggling (unseen), now isn't: due again right away
    expect(first.card.dueAt).toBe(now.toISOString());

    const later = new Date('2026-11-02T12:00:00.000Z');
    const second = rate(s, first, Rating.Good, later);
    expect(second.card.reps).toBe(2);
    expect(new Date(second.card.dueAt).getTime()).toBeGreaterThan(later.getTime());
  });

  it('marks Hard and Again as struggling', async () => {
    const s = await getScheduler();
    const hard = rate(s, undefined, Rating.Hard, now);
    expect(isStruggling(hard)).toBe(true);
    expect(isStruggling(undefined)).toBe(true);
    expect(isStruggling(rate(s, hard, Rating.Easy, now))).toBe(false);
  });
});

describe('nextCard', () => {
  const state = (dueAt: string): ReviewState => ({ lastRating: Rating.Good, card: { dueAt } });

  it('prefers due cards, else unseen, never the last one', () => {
    const states = new Map([['a', state('2026-11-01T00:00:00Z')], ['b', state('2026-12-01T00:00:00Z')]]);
    // random < 0.7: due first
    expect(nextCard(cards, states, null, now, () => 0)?.id).toBe('a');
    // random >= 0.7: unseen first
    expect(nextCard(cards, states, null, now, () => 0.9)?.id).toBe('c');
    // the due one was just shown: fall back to unseen
    expect(nextCard(cards, states, 'a', now, () => 0)?.id).toBe('c');
  });

  it('returns null when nothing is due or unseen', () => {
    const states = new Map(cards.map((c) => [c.id, state('2027-01-01T00:00:00Z')]));
    expect(nextCard(cards, states, null, now)).toBeNull();
  });
});
