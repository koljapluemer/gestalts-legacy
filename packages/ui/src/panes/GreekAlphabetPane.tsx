import { GreekAlphabet } from '../perspectives';

export function GreekAlphabetPane() {
  return (
    <div className="h-full overflow-y-auto px-4 py-8">
      <GreekAlphabet deckTag="greek-alphabet" />
    </div>
  );
}
