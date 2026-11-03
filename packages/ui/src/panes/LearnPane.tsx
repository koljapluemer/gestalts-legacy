import { Learn } from '../perspectives';
import { useOverlay } from './overlay';
import { SearchLayout } from './SearchLayout';

/** The flashcards of every note by spaced repetition; a card's note opens in the editor over it. */
export function LearnPane() {
  const [, overlays] = useOverlay();
  return (
    <SearchLayout>
      <div className="h-full overflow-y-auto px-4 py-8">
        <Learn onOpen={(key) => overlays.open({ kind: 'note', key })} />
      </div>
    </SearchLayout>
  );
}
