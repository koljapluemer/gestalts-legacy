import type { ComponentType, ReactNode } from 'react';
import { useData, useRepo, type IndexedNote, type Query, type Repo } from '@gestalts/client';

/** What a perspective works with: named queries, resolved to notes and kept current. */
export type DataSpec = Record<string, Query>;

export type PerspectiveProps<D extends DataSpec> = {
  data: { [K in keyof D]: IndexedNote[] };
  repo: Repo;
};

type Definition<D extends DataSpec, P extends object> = {
  id: string;
  title: string;
  /** may depend on the perspective's own props */
  data: D | ((props: P) => D);
  Component: (props: P & PerspectiveProps<D>) => ReactNode;
};

export type Perspective<P extends object> = ComponentType<P> & { id: string; title: string };

/**
 * A perspective: a self-contained view on notes. Its ports are
 * - `data`: the notes it reads, declared up front (by gestalt and tags)
 * - `repo`: writes (and blob access)
 * - its own props: inputs and events, wired by the pane that hosts it
 */
export function definePerspective<D extends DataSpec, P extends object = object>(def: Definition<D, P>): Perspective<P> {
  const { Component } = def;
  function Host(props: P) {
    const repo = useRepo();
    const spec = typeof def.data === 'function' ? def.data(props) : def.data;
    const data = useData(spec) as PerspectiveProps<D>['data'];
    return <Component {...props} data={data} repo={repo} />;
  }
  Host.displayName = `Perspective(${def.id})`;
  return Object.assign(Host, { id: def.id, title: def.title });
}
