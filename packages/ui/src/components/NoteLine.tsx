import { noteLabel, type IndexedNote } from '@gestalts/client';
import { HighlightedText } from '../primitives';

/** A note in one line: its label, with search terms highlighted. */
export function NoteLine({ note, terms = [] }: { note: IndexedNote; terms?: string[] }) {
  return (
    <span className="block truncate">
      <HighlightedText text={noteLabel(note)} terms={terms} />
    </span>
  );
}
