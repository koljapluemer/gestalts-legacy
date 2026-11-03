import { useState } from 'react';
import { LoaderCircle, Trash2, X } from 'lucide-react';
import { merge3, noteLabel, type IndexedNote } from '@gestalts/client';
import { GestaltBadges, YamlEditor } from '../components';
import { draftSave } from './draftMerge';

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
 * While there are no unsaved edits it follows the note (e.g. after a sync). If the note changed while a draft was
 * open (a canvas or cards save, a sync), saving merges the draft in per top-level key; if both changed the same key,
 * it asks before writing over the other change.
 */
export function EditableNote({ note, inspect, save, onClose, onDelete, autofocus }: Props) {
  // null: no unsaved edits, show the stored text
  const [draft, setDraft] = useState<string | null>(null);
  // the note's text when the draft started
  const [base, setBase] = useState<string | null>(null);
  const [state, setState] = useState<{ saving: boolean; error?: string; conflict?: boolean }>({ saving: false });
  const text = draft ?? note.raw;
  const live = draft === null ? note : inspect(draft);

  const write = async (raw: string) => {
    setState({ saving: true });
    try {
      await save(raw);
      setDraft(null);
      setBase(null);
      setState({ saving: false });
    } catch (e) {
      setState({ saving: false, error: e instanceof Error ? e.message : String(e) });
    }
  };
  const discard = () => { setDraft(null); setBase(null); setState({ saving: false }); };
  const commit = async () => {
    if (draft === null || draft === note.raw) { discard(); return; }
    const plan = draftSave(base ?? note.raw, draft, note.raw, merge3);
    if (plan.kind === 'conflict') { setState({ saving: false, conflict: true }); return; }
    await write(plan.text);
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
        <YamlEditor value={text} inspect={inspect} onChange={(t) => { if (draft === null) setBase(note.raw); setDraft(t); }} onSave={() => void commit()} autofocus={autofocus} />
        {state.conflict && draft !== null && (
          <div role="alert" className="alert alert-warning alert-soft py-2 text-sm">
            <span className="flex-1">This note was changed elsewhere meanwhile, in the same place as your edit. Not saved.</span>
            <button type="button" className="btn btn-xs" onClick={() => void write(draft)}>Overwrite</button>
            <button type="button" className="btn btn-xs" onClick={discard}>Discard my edits</button>
          </div>
        )}
        {state.error && <p className="text-sm text-error">{state.error}</p>}
      </div>
    </section>
  );
}
