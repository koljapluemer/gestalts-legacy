import { noteLabel, relationships, type Query } from '@gestalts/client';
import { definePerspective } from '../define/definePerspective';
import { CANVAS_OF } from './canvasNote';

type Props = {
  /** a host note was picked: its key */
  onOpen: (key: string) => void;
  /** the key of the host open now */
  current?: string;
};

/** The notes that have a canvas, by label. */
export const CanvasList = definePerspective<{ canvases: Query }, Props>({
  id: 'canvas-list',
  title: 'Canvases',
  data: { canvases: { tagsAll: ['canvas'] } },
  Component: ({ data, repo, onOpen, current }) => {
    const hosts = data.canvases
      .flatMap((c) => relationships(c).filter((r) => r.label === CANVAS_OF).map((r) => repo.keyOf(r.target)))
      .flatMap((key) => { const n = key ? repo.get(key) : undefined; return n ? [n] : []; })
      .sort((a, b) => noteLabel(a).localeCompare(noteLabel(b)));
    if (!hosts.length) return <p className="p-3 text-sm text-base-content/60">No canvases yet. Open any note as a canvas from the search.</p>;
    return (
      <ul className="menu menu-sm w-full">
        {hosts.map((n) => (
          <li key={n.key}>
            <button type="button" className={`block truncate text-left ${n.key === current ? 'menu-active' : ''}`} onClick={() => onOpen(n.key)}>{noteLabel(n)}</button>
          </li>
        ))}
      </ul>
    );
  },
});
