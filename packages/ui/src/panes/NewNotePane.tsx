import { useState } from 'react';
import { NewNote } from '../perspectives';
import { useOverlay } from './overlay';
import { SearchLayout } from './SearchLayout';

type Form = { title?: string; tags?: string[]; seq: number };

/** The new-note form on its own; the search's "+" refills it. A created note opens in the editor over the pane. */
export function NewNotePane() {
  const [form, setForm] = useState<Form>({ seq: 0 });
  const [, overlays] = useOverlay();
  const reset = (prefill: Omit<Form, 'seq'> = {}) => setForm((f) => ({ ...prefill, seq: f.seq + 1 }));
  return (
    <SearchLayout onCreate={reset}>
      <div className="h-full overflow-y-auto">
        <div className="mx-auto max-w-2xl py-4">
          <NewNote
            key={form.seq}
            title={form.title}
            tags={form.tags}
            onCreated={(key) => { reset(); overlays.open({ kind: 'note', key }); }}
          />
        </div>
      </div>
    </SearchLayout>
  );
}
