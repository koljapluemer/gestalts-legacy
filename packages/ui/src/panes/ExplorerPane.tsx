import { UniversalEditor } from '../perspectives';
import { SearchLayout } from './SearchLayout';
import { useUrlList } from './urlState';

/** The search on the right (a drawer on small screens) opens notes in the editor. */
export function ExplorerPane() {
  const [open, setOpen] = useUrlList('open');
  const show = (key: string) => setOpen([key, ...open.filter((k) => k !== key)]);
  return (
    <SearchLayout onOpen={show} openKeys={open}>
      <div className="h-full overflow-y-auto">
        <UniversalEditor open={open} onClose={(key) => setOpen(open.filter((k) => k !== key))} onOpen={show} />
      </div>
    </SearchLayout>
  );
}
