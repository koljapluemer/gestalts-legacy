import { bibCitedKeys, bibMatches, bibMerge, bibModel, bibParse, bibUniqueCitekey, bibWrite } from '../wasm/gestalts.js';
import type { Json } from './types';

/**
 * Bibliographic sources (`source@1`) and BibTeX, from the core (`crates/core/src/bib.rs`).
 * Call only once a repo is open: that loads the core.
 */

/** A source as a note's map: `title`, `citekey`, `entrytype`, biblatex fields, `bib_extra`. */
export type Source = { [field: string]: Json };
export type FieldKind = 'names' | 'list' | 'date' | 'verbatim' | 'literal';
export type SourceMatch = { key: string; level: 'same' | 'likely' | 'possible'; reasons: string[] };
export type SourceMerge = { source: Source; filled: string[]; conflicts: { field: string; current: Json; incoming: Json }[] };

export const bib = {
  /** The entries of BibTeX text; throws when there are none. */
  parse: (text: string) => bibParse(text) as { source: Source; warnings: string[] }[],
  /** Sources as biblatex text, by citekey. */
  write: (sources: Source[]) => bibWrite(sources),
  /** The existing sources that may be `entry`, best first. */
  matches: (entry: Source, existing: { key: string; source: Source }[], limit = 3) => bibMatches(entry, existing, limit) as SourceMatch[],
  /** `current` updated from `incoming`; fields named in `take` are replaced, other differences are conflicts. */
  merge: (current: Source, incoming: Source, take: string[] = []) => bibMerge(current, incoming, take) as SourceMerge,
  uniqueCitekey: (key: string, taken: string[]) => bibUniqueCitekey(key, taken),
  /** The keys a Typst document cites, in order. */
  citedKeys: (typst: string) => bibCitedKeys(typst),
  model: () => bibModel() as { entryTypes: [string, string[]][]; fields: [string, FieldKind][] },
};
