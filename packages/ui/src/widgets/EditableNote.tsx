import { useState } from 'react';
import { LoaderCircle, Trash2, X } from 'lucide-react';
import { noteLabel, type IndexedNote } from '@gestalts/client';
import { GestaltBadges, YamlEditor } from '../components';

type Props = {
  note: IndexedNote;
  inspect: (raw: string) => IndexedNote;
  save: (raw: string) => Promise<unknown>;
  onClose?: () => void;
  onDelete?: () => void;
  autofocus?: boolean;
};

/**
 * One note as raw YAML. Saves when focus leaves it and on Mod-s.
 * While there are no unsaved edits it follows the note (e.g. after a sync).
 */
export function EditableNote({ note, inspect, save, onClose, onDelete, autofocus }: Props) {
  // null: no unsaved edits, show the stored text
  const [draft, setDraft] = useState<string | null>(null);
  const [state, setState] = useState<{ saving: boolean; error?: string }>({ saving: false });
  const text = draft ?? note.raw;
  const live = draft === null ? note : inspect(draft);

  const commit = async () => {
    if (draft === null || draft === note.raw) { setDraft(null); return; }
    setState({ saving: true });
    try {
      await save(draft);
      setDraft(null);
      setState({ saving: false });
    } catch (e) {
      setState({ saving: false, error: e instanceof Error ? e.message : String(e) });
    }
  };

  return (
    <section
      className="card card-border bg-base-100"
      onBlur={(e) => { if (!e.currentTarget.contains(e.relatedTarget)) void commit(); }}
    >
      <div className="card-body gap-2 p-3">
        <header className="flex items-start gap-2">
          <div className="min-w-0 flex-1">
            <h2 className="truncate font-semibold">{noteLabel(live)}</h2>
            <p className="truncate font-mono text-xs text-base-content/50">{note.key}</p>
          </div>
          {state.saving && <LoaderCircle className="size-4 animate-spin" />}
          {draft !== null && !state.saving && <span className="badge badge-sm badge-ghost">unsaved</span>}
          {onDelete && (
            <button type="button" className="btn btn-ghost btn-xs btn-square" title="Delete note" onClick={onDelete}>
              <Trash2 className="size-4" />
            </button>
          )}
          {onClose && (
            <button type="button" className="btn btn-ghost btn-xs btn-square" title="Close" onClick={() => { void commit(); onClose(); }}>
              <X className="size-4" />
            </button>
          )}
        </header>
        <GestaltBadges gestalts={live.gestalts} lints={live.lints} />
        <YamlEditor value={text} inspect={inspect} onChange={(t) => setDraft(t)} onSave={() => void commit()} autofocus={autofocus} />
        {state.error && <p className="text-sm text-error">{state.error}</p>}
      </div>
    </section>
  );
}
