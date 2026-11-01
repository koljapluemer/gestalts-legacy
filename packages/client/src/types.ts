/** Shapes handed out by the core (see crates/core). */

export type Json = null | boolean | number | string | Json[] | { [key: string]: Json };

export type ParseError = { message: string; line: number | null; column: number | null };

export type Lint = {
  /** `name@version` */
  gestalt: string;
  /** e.g. `card.dueAt`, `blobs.audio`; empty for the whole note */
  path: string;
  message: string;
};

export type IndexedNote = {
  /** file name without `.yml` */
  key: string;
  /** the `id` field, else the key */
  id: string;
  raw: string;
  /** the parsed YAML, if it parses */
  value: Json | null;
  parse_error: ParseError | null;
  /** `name@version` of every gestalt the note has */
  gestalts: string[];
  lints: Lint[];
};

export type Query = {
  /** `name@version` */
  gestalt?: string;
  tagsAll?: string[];
  tagsNone?: string[];
  keys?: string[];
};

export type GestaltDef = {
  name: string;
  version: number;
  description: string | null;
  requires_tags: string[];
  trigger_fields: string[];
  fields: Record<string, unknown>;
  blobs: Record<string, { min: number | null; max: number | null }>;
  relationships: Record<string, { min: number | null; max: number | null }>;
};

export type SyncReport = {
  pulled: string[];
  pushed: string[];
  conflicts: string[];
  blobs_pulled: string[];
  blobs_pushed: string[];
};

export type Ref = { target: string; label: string | null };
