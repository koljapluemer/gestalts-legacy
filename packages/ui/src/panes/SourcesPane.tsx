import { BookOpen } from 'lucide-react';
import { tags } from '@gestalts/client';
import { Sources } from '../perspectives';
import { useOverlay } from './overlay';
import { SearchLayout } from './SearchLayout';
import { useUrlList } from './urlState';

/** Bibliographic sources: the list and the one open; a source found in the search opens here. */
export function SourcesPane() {
  const [[open] = [], setOpen] = useUrlList('source');
  const [, overlays] = useOverlay();
  const edit = (key: string) => overlays.open({ kind: 'note', key });
  return (
    <SearchLayout openKeys={open ? [open] : []} actions={[
      { icon: <BookOpen className="size-3.5" />, title: 'Open in Sources', run: (k) => setOpen([k]), applies: (n) => tags(n).includes('source') },
    ]}>
      <Sources open={open} onSelect={(k) => setOpen(k ? [k] : [])} onOpen={edit} />
    </SearchLayout>
  );
}
