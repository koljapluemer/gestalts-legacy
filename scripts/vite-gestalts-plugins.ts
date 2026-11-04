/**
 * The plugins an app is built with (`distribution.yml` next to its `vite.config.ts`), as the virtual module
 * `virtual:gestalts-plugins`: each plugin's manifest, its gestalts (YAML text), its deck (served under
 * `decks/<id>/`) and loaders for its UI modules. The host loads plugins through this registry and never imports
 * them, so an app builds with any list, even an empty one.
 */
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative, resolve } from 'node:path';
import type { Plugin } from 'vite';
import { parse } from 'yaml';

const VIRTUAL = 'virtual:gestalts-plugins';
const RESOLVED = '\0' + VIRTUAL;
const PLUGINS = resolve(import.meta.dirname, '../plugins');

type Manifest = {
  id: string;
  host_api: number;
  gestalts?: string[];
  deck?: { path: string; aliases?: string[] };
  panes?: { id: string; title: string; entry: string }[];
  slots?: { slot: string; gestalt: string; entry: string }[];
};

type Distribution = { plugins: string[]; default_panes?: string[]; install_ui?: boolean };

function files(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const p = join(dir, name);
    return statSync(p).isDirectory() ? files(p) : [p];
  });
}

function load(appRoot: string) {
  const distribution = parse(readFileSync(join(appRoot, 'distribution.yml'), 'utf8')) as Distribution;
  const plugins = (distribution.plugins ?? []).map((id) => {
    const root = join(PLUGINS, id);
    const manifest = parse(readFileSync(join(root, 'plugin.yml'), 'utf8')) as Manifest;
    if (manifest.id !== id) throw new Error(`plugins/${id}/plugin.yml has id ${manifest.id}`);
    if (manifest.host_api !== 1) throw new Error(`${id} needs host API ${manifest.host_api}; this host offers 1`);
    const gestalts = (manifest.gestalts ?? []).map((g) => {
      const [name, version] = g.split('@');
      return readFileSync(join(root, 'gestalts', name, `v${version}.yml`), 'utf8');
    });
    const deckDir = manifest.deck ? join(root, manifest.deck.path) : null;
    if (deckDir && !existsSync(join(deckDir, 'index.json'))) throw new Error(`${id}: its deck has no index.json; run \`mise run publish-data ${relative(process.cwd(), deckDir)}\``);
    const modules = [...new Set([...(manifest.panes ?? []), ...(manifest.slots ?? [])].map((e) => e.entry.split('#')[0]))];
    return { id, root, manifest, gestalts, deckDir, modules };
  });
  for (const ref of distribution.default_panes ?? []) {
    const [plugin, pane] = ref.split('/');
    if (!plugins.some((p) => p.id === plugin && p.manifest.panes?.some((q) => q.id === pane))) throw new Error(`default pane ${ref} is in no plugin of the distribution`);
  }
  return { distribution, plugins };
}

export function gestaltsPlugins(): Plugin {
  let appRoot = process.cwd();
  return {
    name: 'gestalts-plugins',
    configResolved(config) { appRoot = config.root; },
    resolveId(id) { return id === VIRTUAL ? RESOLVED : undefined; },
    load(id) {
      if (id !== RESOLVED) return;
      const { distribution, plugins } = load(appRoot);
      this.addWatchFile(join(appRoot, 'distribution.yml'));
      const entries = plugins.map((p) => {
        this.addWatchFile(join(p.root, 'plugin.yml'));
        const modules = p.modules.map((m) => `${JSON.stringify(m)}: () => import(${JSON.stringify(join(p.root, m))})`).join(', ');
        const deck = p.deckDir ? { source: p.id, aliases: p.manifest.deck?.aliases ?? [], path: `decks/${p.id}` } : null;
        return `{ manifest: ${JSON.stringify(p.manifest)}, gestalts: ${JSON.stringify(p.gestalts)}, deck: ${JSON.stringify(deck)}, modules: { ${modules} } }`;
      });
      return `export const distribution = ${JSON.stringify({ default_panes: [], install_ui: false, ...distribution })};\nexport const plugins = [${entries.join(',\n')}];\n`;
    },
    configureServer(server) {
      // decks in development: /decks/<id>/… from plugins/<id>/<deck path>
      server.middlewares.use((req, res, next) => {
        const m = /^\/decks\/([a-z0-9-]+)\/(.+)$/.exec((req.url ?? '').split('?')[0]);
        if (!m) return next();
        const p = load(appRoot).plugins.find((x) => x.id === m[1]);
        const file = p?.deckDir && resolve(p.deckDir, decodeURIComponent(m[2]));
        if (!file || !file.startsWith(p.deckDir + '/') || !existsSync(file)) { res.statusCode = 404; res.end(); return; }
        res.end(readFileSync(file));
      });
    },
    generateBundle() {
      for (const p of load(appRoot).plugins) {
        if (!p.deckDir) continue;
        for (const f of files(p.deckDir)) {
          this.emitFile({ type: 'asset', fileName: `decks/${p.id}/${relative(p.deckDir, f)}`, source: readFileSync(f) });
        }
      }
    },
  };
}
