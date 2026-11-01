import { FileQuestion } from 'lucide-react';
import { useNote, type Repo } from '@gestalts/client';
import { EditableNote } from '../../widgets';
import { definePerspective } from '../define/definePerspective';

type Props = {
  /** keys of the open notes, newest first */
  open: string[];
  onClose: (key: string) => void;
};

/** Any note as raw YAML, with live gestalt lints. Several can be open at once. */
export const UniversalEditor = definePerspective<Record<string, never>, Props>({
  id: 'universal-editor',
  title: 'Editor',
  data: {},
  Component: ({ open, onClose, repo }) => {
    if (!open.length) {
      return <p className="p-8 text-center text-base-content/60">Open a note from the search, or create one.</p>;
    }
    return (
      <div className="flex flex-col gap-3 p-3">
        {open.map((key, i) => <OpenNote key={key} noteKey={key} autofocus={i === 0} onClose={() => onClose(key)} repo={repo} />)}
      </div>
    );
  },
});

function OpenNote({ noteKey, autofocus, onClose, repo }: { noteKey: string; autofocus: boolean; onClose: () => void; repo: Repo }) {
  const note = useNote(noteKey);
  if (!note) {
    return (
      <div role="alert" className="alert alert-soft">
        <FileQuestion className="size-4" />
        <span className="flex-1">No note <code>{noteKey}</code> (deleted, or not loaded).</span>
        <button type="button" className="btn btn-xs" onClick={onClose}>Close</button>
      </div>
    );
  }
  return (
    <EditableNote
      note={note}
      autofocus={autofocus}
      inspect={(raw) => repo.inspect(noteKey, raw)}
      save={(raw) => repo.putRaw(noteKey, raw)}
      onClose={onClose}
      onDelete={() => {
        if (!confirm(`Delete this note?\n${noteKey}`)) return;
        void repo.delete(noteKey).then(onClose);
      }}
    />
  );
}
