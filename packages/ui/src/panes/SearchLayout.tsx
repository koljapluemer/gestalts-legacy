import { useState, type ReactNode } from 'react';
import { PanelRight } from 'lucide-react';
import { Modal } from './Modal';
import { NewNote, SearchSidebar, UniversalEditor, type NoteAction } from '../perspectives';
import { useOverlay } from './overlay';

type Prefill = { title?: string; tags: string[] };

type Props = {
  children: ReactNode;
  /** what results offer besides opening them */
  actions?: NoteAction[];
  /** a result was opened (or a note created); by default it opens in an editor over the pane */
  onOpen?: (key: string) => void;
  /** the "+" was pressed; by default a new-note form opens over the pane */
  onCreate?: (prefill: Prefill) => void;
  /** keys shown marked in the results */
  openKeys?: string[];
};

/**
 * A pane's content with the search on the right (a drawer on small screens), and at most one overlay over it
 * (see `useOverlay`): a note in the editor, or the new-note form.
 */
export function SearchLayout({ children, actions, onOpen, onCreate, openKeys }: Props) {
  const [drawer, setDrawer] = useState(false);
  const [overlay, overlays] = useOverlay();
  const open = (key: string) => { setDrawer(false); if (onOpen) onOpen(key); else overlays.open({ kind: 'note', key }); };
  const create = (prefill: Prefill) => { setDrawer(false); if (onCreate) onCreate(prefill); else overlays.open({ kind: 'new', ...prefill }); };
  const created = (key: string) => {
    // in place, not `close`: going back is async and would undo what `onOpen` writes to the URL
    if (onOpen) { overlays.replace(null); onOpen(key); } else overlays.replace({ kind: 'note', key });
  };
  const act = (actions ?? []).map((a) => ({ ...a, run: (key: string) => { setDrawer(false); a.run(key); } }));
  return (
    <div className="drawer drawer-end h-full lg:drawer-open">
      <input id="pane-search" type="checkbox" className="drawer-toggle" checked={drawer} onChange={(e) => setDrawer(e.target.checked)} />
      <div className="drawer-content flex min-h-0 flex-col">
        <div className="flex justify-end px-3 pt-2 lg:hidden">
          <label htmlFor="pane-search" className="btn btn-sm btn-ghost gap-1"><PanelRight className="size-4" /> Search</label>
        </div>
        <div className="min-h-0 flex-1">{children}</div>
      </div>
      <div className="drawer-side z-40 h-full lg:h-auto">
        <label htmlFor="pane-search" aria-label="Close search" className="drawer-overlay" />
        <aside className="h-full w-[22rem] max-w-[85vw] border-l border-base-300 bg-base-100">
          <SearchSidebar onOpen={open} onCreate={create} openKeys={openKeys} actions={act} />
        </aside>
      </div>
      {overlay && (
        <Modal onClose={overlays.close}>
          {overlay.kind === 'note'
            ? <UniversalEditor key={overlay.key} open={[overlay.key]} onClose={overlays.close} />
            : <NewNote key={JSON.stringify(overlay)} title={overlay.title} tags={overlay.tags} onCreated={created} onCancel={overlays.close} />}
        </Modal>
      )}
    </div>
  );
}
