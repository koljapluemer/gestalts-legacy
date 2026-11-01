import { useState } from 'react';
import { PanelRight } from 'lucide-react';
import { SearchSidebar, UniversalEditor } from '../perspectives';
import { useUrlList } from './urlState';

/** The search on the right (a drawer on small screens) opens notes in the editor. */
export function ExplorerPane() {
  const [open, setOpen] = useUrlList('open');
  const [drawer, setDrawer] = useState(false);
  const openNote = (key: string) => {
    setOpen([key, ...open.filter((k) => k !== key)]);
    setDrawer(false);
  };
  return (
    <div className="drawer drawer-end h-full lg:drawer-open">
      <input id="explorer-search" type="checkbox" className="drawer-toggle" checked={drawer} onChange={(e) => setDrawer(e.target.checked)} />
      <div className="drawer-content min-h-0 overflow-y-auto">
        <div className="flex justify-end px-3 pt-3 lg:hidden">
          <label htmlFor="explorer-search" className="btn btn-sm btn-ghost gap-1"><PanelRight className="size-4" /> Search</label>
        </div>
        <UniversalEditor open={open} onClose={(key) => setOpen(open.filter((k) => k !== key))} />
      </div>
      <div className="drawer-side z-40 h-full lg:h-auto">
        <label htmlFor="explorer-search" aria-label="Close search" className="drawer-overlay" />
        <aside className="h-full w-[22rem] max-w-[85vw] border-l border-base-300 bg-base-100">
          <SearchSidebar onOpen={openNote} openKeys={open} />
        </aside>
      </div>
    </div>
  );
}
