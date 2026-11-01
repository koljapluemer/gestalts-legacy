import type { IndexedNote } from '@gestalts/client';
import { NoteLine } from '../components';

type Props = {
  notes: IndexedNote[];
  terms: string[];
  /** index of the keyboard-selected result */
  active: number;
  /** keys shown as already open */
  marked?: Set<string>;
  onOpen: (key: string) => void;
};

/** Search results; keyboard navigation is driven by the owner of `active`. */
export function NoteResultList({ notes, terms, active, marked, onOpen }: Props) {
  if (!notes.length) return <p className="px-3 py-6 text-center text-sm text-base-content/60">No notes found.</p>;
  return (
    <ul className="menu menu-sm w-full p-0">
      {notes.map((n, i) => (
        <li key={n.key}>
          <button
            type="button"
            // keep focus in the search field
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => onOpen(n.key)}
            className={[
              'block w-full text-left',
              i === active ? 'menu-active' : '',
              marked?.has(n.key) ? 'border-l-2 border-primary/60' : '',
            ].join(' ')}
          >
            <NoteLine note={n} terms={terms} />
            {n.gestalts.length > 0 && <span className="block truncate text-xs text-base-content/50">{n.gestalts.join(' · ')}</span>}
          </button>
        </li>
      ))}
    </ul>
  );
}
