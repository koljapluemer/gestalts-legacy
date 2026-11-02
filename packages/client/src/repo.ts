import init, { WasmRepo, newId as wasmNewId } from '../wasm/gestalts.js';
import type { GestaltDef, IndexedNote, Json, Query, SyncReport, UpstreamReport } from './types';

export type RepoSource =
  /** notes live in OPFS under `dir` */
  | { kind: 'opfs'; dir: string }
  /** read-only notes published at `staticBase` (a URL), copied into OPFS under `dir` once the user's notes reference them */
  | { kind: 'layered'; staticBase: string; dir: string };

export type ConnectorStatus = {
  url: string | null;
  state: 'off' | 'syncing' | 'ok' | 'error';
  error?: string;
  lastSync?: Date;
  lastReport?: SyncReport;
  /** conflict notes created since connecting */
  conflicts: string[];
};

/** A versioned optional HTTP service exposed by a connector-hosted plugin. */
export class ServiceClient {
  constructor(readonly baseUrl: string, readonly id: string) {}

  request(path = '', init?: RequestInit): Promise<Response> {
    const suffix = path.replace(/^\//, '');
    return fetch(`${this.baseUrl}/services/${encodeURIComponent(this.id)}/${suffix}`, init);
  }
}

let ready: Promise<unknown> | null = null;

/**
 * The notes and blobs an app works with, backed by the Rust core.
 * Subscribe to learn about any change (own writes or a sync); `version` increases on each.
 */
export class Repo {
  private listeners = new Set<() => void>();
  private queue: Promise<unknown> = Promise.resolve();
  private blobUrls = new Map<string, Promise<string | null>>();
  private status: ConnectorStatus = { url: null, state: 'off', conflicts: [] };
  private statusListeners = new Set<() => void>();
  private syncTimer: ReturnType<typeof setTimeout> | undefined;
  private pollTimer: ReturnType<typeof setInterval> | undefined;
  version = 0;

  private constructor(private wasm: WasmRepo, private storageKey: string) {}

  static async open(source: RepoSource): Promise<Repo> {
    ready ??= init();
    await ready;
    const wasm = source.kind === 'opfs' ? await WasmRepo.opfs(source.dir) : await WasmRepo.layered(source.staticBase, source.dir);
    const repo = new Repo(wasm, `gestalts-connector:${source.dir}`);
    let saved: string | null = null;
    try { saved = localStorage.getItem(repo.storageKey); } catch { /* no storage: start disconnected */ }
    if (saved) void repo.connect(saved);
    return repo;
  }

  subscribe = (l: () => void) => {
    this.listeners.add(l);
    return () => { this.listeners.delete(l); };
  };

  private changed() {
    this.version++;
    this.listeners.forEach((l) => l());
  }

  /** Runs mutations one at a time, so a sync never interleaves with a write. */
  private serial<T>(f: () => Promise<T>): Promise<T> {
    const run = this.queue.then(f, f);
    this.queue = run.catch(() => undefined);
    return run;
  }

  query(q: Query = {}): IndexedNote[] {
    return this.wasm.query(q) as IndexedNote[];
  }

  get(key: string): IndexedNote | undefined {
    return (this.wasm.get(key) as IndexedNote | null) ?? undefined;
  }

  /** The key of the note with this `id` (or key). */
  keyOf(id: string): string | undefined {
    return this.wasm.keyOf(id);
  }

  /** For a layered repo: what the last check against upstream (on open, after each sync) did. */
  upstreamReport(): UpstreamReport {
    return this.wasm.upstreamReport() as UpstreamReport;
  }

  gestalts(): GestaltDef[] {
    return this.wasm.gestalts() as GestaltDef[];
  }

  /** Returns undefined while offline. A declared service may still answer 503 while its worker starts. */
  service(id: string): ServiceClient | undefined {
    return this.status.url ? new ServiceClient(this.status.url.replace(/\/$/, ''), id) : undefined;
  }

  /** What `raw` would index as, without saving it. */
  inspect(key: string, raw: string): IndexedNote {
    return this.wasm.inspect(key, raw) as IndexedNote;
  }

  async putRaw(key: string, raw: string): Promise<IndexedNote> {
    const n = await this.serial(() => this.wasm.putRaw(key, raw) as Promise<IndexedNote>);
    this.afterWrite();
    return n;
  }

  /** Without a key, a new uuid becomes key and `id`. */
  async putValue(key: string | undefined, value: Json): Promise<IndexedNote> {
    const n = await this.serial(() => this.wasm.putValue(key, JSON.stringify(value)) as Promise<IndexedNote>);
    this.afterWrite();
    return n;
  }

  async delete(key: string): Promise<void> {
    await this.serial(() => this.wasm.delete(key));
    this.afterWrite();
  }

  async readBlob(name: string): Promise<Uint8Array | undefined> {
    return this.wasm.readBlob(name);
  }

  async writeBlob(name: string, bytes: Uint8Array): Promise<void> {
    await this.serial(() => this.wasm.writeBlob(name, bytes));
    this.blobUrls.delete(name);
    this.afterWrite();
  }

  /** An object URL for a blob, cached; `null` if missing. */
  blobUrl(name: string): Promise<string | null> {
    let url = this.blobUrls.get(name);
    if (!url) {
      url = this.readBlob(name).then((b) => (b ? URL.createObjectURL(new Blob([b as Uint8Array<ArrayBuffer>], { type: mimeOf(name) })) : null));
      this.blobUrls.set(name, url);
    }
    return url;
  }

  private afterWrite() {
    this.changed();
    if (this.status.url) {
      clearTimeout(this.syncTimer);
      this.syncTimer = setTimeout(() => void this.sync(), 800);
    }
  }

  // --- connector ---

  connector = {
    get: () => this.status,
    subscribe: (l: () => void) => {
      this.statusListeners.add(l);
      return () => { this.statusListeners.delete(l); };
    },
  };

  private setStatus(patch: Partial<ConnectorStatus>) {
    this.status = { ...this.status, ...patch };
    this.statusListeners.forEach((l) => l());
  }

  /** Syncs with the connector at `url` now, then after each write and every 15 s. */
  async connect(url: string): Promise<void> {
    try { localStorage.setItem(this.storageKey, url); } catch { /* not remembered */ }
    clearInterval(this.pollTimer);
    this.setStatus({ url, state: 'syncing', error: undefined, conflicts: [] });
    this.pollTimer = setInterval(() => void this.sync(), 15_000);
    await this.sync();
  }

  disconnect() {
    try { localStorage.removeItem(this.storageKey); } catch { /* nothing remembered */ }
    clearInterval(this.pollTimer);
    clearTimeout(this.syncTimer);
    this.setStatus({ url: null, state: 'off', error: undefined });
  }

  async sync(): Promise<SyncReport | undefined> {
    const url = this.status.url;
    if (!url) return;
    this.setStatus({ state: 'syncing' });
    try {
      const tag = new Date().toISOString().replace(/[:.]/g, '-');
      const report = await this.serial(() => this.wasm.sync(url, tag) as Promise<SyncReport>);
      if (this.status.url !== url) return report;
      this.setStatus({ state: 'ok', lastSync: new Date(), lastReport: report, conflicts: [...this.status.conflicts, ...report.conflicts] });
      if (report.pulled.length || report.blobs_pulled.length) {
        report.blobs_pulled.forEach((b) => this.blobUrls.delete(b));
        this.changed();
      }
      return report;
    } catch (e) {
      this.setStatus({ state: 'error', error: e instanceof Error ? e.message : String(e) });
    }
  }
}

export function newId(): string {
  return wasmNewId();
}

const MIME: Record<string, string> = {
  webp: 'image/webp', png: 'image/png', jpg: 'image/jpeg', jpeg: 'image/jpeg', gif: 'image/gif', svg: 'image/svg+xml',
  mp3: 'audio/mpeg', ogg: 'audio/ogg', wav: 'audio/wav', m4a: 'audio/mp4', mp4: 'video/mp4', webm: 'video/webm', pdf: 'application/pdf',
};

function mimeOf(name: string): string {
  return MIME[name.split('.').pop()?.toLowerCase() ?? ''] ?? 'application/octet-stream';
}
