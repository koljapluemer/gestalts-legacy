import { useState } from 'react';
import { FilePlus2, FileType, PanelLeft, SquarePlus } from 'lucide-react';
import { noteLabel, useNote, useQuery, useRepo } from '@gestalts/client';
import { Canvas, OLD_CANVASES, TypstEditor, TypstHosts, canvasLinksOf, createTypstDocument, documentsOf } from '../perspectives';
import { useOverlay } from './overlay';
import { SearchLayout } from './SearchLayout';
import { useUrlList } from './urlState';

/**
 * A note's canvas beside its Typst document, for writing from what's gathered on the canvas. From the search any
 * note opens here, given a new Typst document first unless it has one.
 */
export function Canvas2TypPane() {
  const repo = useRepo();
  const [[host] = [], setHost] = useUrlList('host');
  const [[picked] = [], setPicked] = useUrlList('doc');
  const [listing, setListing] = useState(!host);
  const [side, setSide] = useState<'canvas' | 'typst'>('canvas');
  const [add, setAdd] = useState<{ key: string; seq: number } | null>(null);
  const [, overlays] = useOverlay();
  const edit = (key: string) => overlays.open({ kind: 'note', key });
  const hostNote = useNote(host);
  const documents = useQuery({ tagsAll: ['typst'] });
  const canvases = useQuery(OLD_CANVASES);
  const docs = hostNote ? documentsOf(documents, hostNote.id) : [];
  const doc = docs.find((d) => d.key === picked) ?? docs[0];
  // sources on the canvas are offered first when citing
  const onCanvas = hostNote ? canvasLinksOf(hostNote, canvases) : [];

  const show = (key: string) => { setHost([key]); setPicked([]); setListing(false); };
  const openHere = async (key: string) => {
    const n = repo.get(key);
    if (!n) return;
    if (!documentsOf(documents, n.id).length) await createTypstDocument(repo, n);
    show(key);
  };

  return (
    <SearchLayout openKeys={host ? [host] : []} actions={[
      { icon: <FileType className="size-3.5" />, title: 'Open as canvas + Typst (a new Typst document unless it has one)', run: (k) => void openHere(k) },
      ...(host ? [{ icon: <SquarePlus className="size-3.5" />, title: 'Add to this canvas', run: (key: string) => setAdd({ key, seq: Date.now() }) }] : []),
    ]}>
      <div className="flex h-full min-h-0 flex-col md:flex-row">
        <aside className={`min-h-0 overflow-y-auto border-base-300 md:w-56 md:border-r ${listing ? '' : 'max-md:hidden'}`}>
          <TypstHosts current={host} onOpen={show} />
        </aside>
        {hostNote ? (
          <section className={`flex min-h-0 flex-1 flex-col ${listing ? 'max-md:hidden' : ''}`}>
            <div className="flex flex-wrap items-center gap-2 border-b border-base-300 px-2 py-1">
              <button type="button" className="btn btn-xs md:hidden" onClick={() => setListing(true)}><PanelLeft className="size-3.5" /> Notes</button>
              <span className="truncate font-semibold">{noteLabel(hostNote)}</span>
              <div role="tablist" className="join xl:hidden">
                {(['canvas', 'typst'] as const).map((s) => (
                  <button key={s} type="button" role="tab" className={`btn btn-xs join-item ${side === s ? 'btn-active' : ''}`} onClick={() => setSide(s)}>
                    {s === 'canvas' ? 'Canvas' : 'Typst'}
                  </button>
                ))}
              </div>
              {docs.length > 1 && (
                <select className="select select-xs" value={doc?.key} onChange={(e) => setPicked([e.target.value])}>
                  {docs.map((d) => <option key={d.key} value={d.key}>{noteLabel(d)}</option>)}
                </select>
              )}
            </div>
            {/* both stay mounted, so switching sides keeps the canvas and unsaved Typst edits */}
            <div className="grid min-h-0 flex-1 grid-cols-1 xl:grid-cols-2">
              <div className={`min-h-0 ${side === 'canvas' ? '' : 'max-xl:hidden'}`}>
                <Canvas host={hostNote.key} onOpen={edit} add={add} />
              </div>
              <div className={`min-h-0 overflow-y-auto border-base-300 p-2 xl:border-l ${side === 'typst' ? '' : 'max-xl:hidden'}`}>
                {doc
                  ? <TypstEditor key={doc.key} document={doc.key} preferredSources={onCanvas} />
                  : (
                    <button type="button" className="btn btn-sm" onClick={() => void openHere(hostNote.key)}>
                      <FilePlus2 className="size-4" /> New Typst document
                    </button>
                  )}
              </div>
            </div>
          </section>
        ) : (
          <p className={`flex-1 p-8 text-center text-base-content/60 ${listing ? 'max-md:hidden' : ''}`}>Pick a note, or open any from the search.</p>
        )}
      </div>
    </SearchLayout>
  );
}
