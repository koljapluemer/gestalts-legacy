import { field, relationships, stableId, type IndexedNote, type Json } from '@gestalts/client';
import { linkedNotes, type WireNode } from './model';

export const CANVAS_OF = 'canvas-of';
export const ON_CANVAS = 'on-canvas';

/** The canvas about the note `hostId`: a `canvas@1` note that is `canvas-of` it. */
export function canvasOf(canvases: IndexedNote[], hostId: string): IndexedNote | undefined {
  return canvases.find((c) => relationships(c).some((r) => r.label === CANVAS_OF && r.target === hostId));
}

/** Where a host's canvas goes when it has none yet: the same key on every device, so two first edits are one note. */
export const newCanvasKey = (hostId: string) => stableId('canvas', hostId);

/**
 * The canvas note's value with these nodes: its other fields kept, tagged `canvas`, `canvas-of` the host and
 * `on-canvas` each linked note (so they travel with it), other relationships kept.
 */
export function canvasValue(existing: Json | null | undefined, key: string, hostId: string, nodes: WireNode[]): Json {
  const base = existing && typeof existing === 'object' && !Array.isArray(existing) ? existing : { id: key };
  const tags = Array.isArray(base.tags) ? base.tags : [];
  const others = (Array.isArray(base.relationships) ? base.relationships : []).filter((r) => {
    const label = r && typeof r === 'object' && !Array.isArray(r) ? r.label : null;
    return label !== CANVAS_OF && label !== ON_CANVAS;
  });
  return {
    ...base,
    tags: tags.includes('canvas') ? tags : [...tags, 'canvas'],
    nodes: nodes.map((n) => ({ ...n })),
    relationships: [
      { to: hostId, label: CANVAS_OF },
      ...linkedNotes(nodes).map((to) => ({ to, label: ON_CANVAS })),
      ...others,
    ],
  };
}

export const storedNodes = (canvas: IndexedNote | undefined) => (canvas ? field(canvas, 'nodes') : undefined);
