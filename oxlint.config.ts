import { defineConfig } from 'oxlint';

/**
 * UI layers (packages/ui/src) may only import from below:
 * panes > perspectives > widgets > components > primitives.
 * Perspectives don't import each other (except the shared `define` slice), and panes see
 * perspectives only through their index.
 * Every layer may use @gestalts/client and external packages.
 */
const ui = 'packages/ui/src';
const below: Record<string, string[]> = {
  primitive: [],
  component: ['primitive'],
  widget: ['component', 'primitive'],
  perspective: ['widget', 'component', 'primitive'],
  'perspective-index': ['perspective'],
  pane: ['perspective-index'],
};

export default defineConfig({
  ignorePatterns: ['**/dist', 'packages/client/wasm'],
  plugins: ['typescript', 'react', 'oxc', 'unicorn'],
  jsPlugins: [{ name: 'react-hooks-js', specifier: 'eslint-plugin-react-hooks' }, 'eslint-plugin-boundaries'],
  categories: { correctness: 'error' },
  settings: {
    'import/resolver': { typescript: { project: ['packages/*/tsconfig.json', 'apps/*/tsconfig.json'] }, node: true },
    'boundaries/elements': [
      { type: 'client', pattern: 'packages/client/src' },
      { type: 'primitive', pattern: `${ui}/primitives` },
      { type: 'component', pattern: `${ui}/components` },
      { type: 'widget', pattern: `${ui}/widgets` },
      { type: 'perspective', pattern: `${ui}/perspectives/*`, capture: ['slice'] },
      // files directly in perspectives/: the index panes import
      { type: 'perspective-index', pattern: `${ui}/perspectives` },
      { type: 'pane', pattern: `${ui}/panes` },
    ],
  },
  rules: {
    'react/rules-of-hooks': 'error',
    'react/exhaustive-deps': 'warn',
    'react-hooks-js/set-state-in-render': 'error',
    'react-hooks-js/immutability': 'error',
    'react-hooks-js/refs': 'error',
  },
  overrides: [{
    files: [`${ui}/**/*.{ts,tsx}`],
    rules: {
      'boundaries/no-unknown-files': 'error',
      'boundaries/dependencies': ['error', {
        default: 'disallow',
        policies: [
          { allow: { dependency: { relationship: { to: 'internal' } } } },
          { allow: { to: { element: { type: 'client' } } } },
          { from: { element: { type: 'perspective' } }, allow: { to: { element: { type: 'perspective', captured: { slice: 'define' } } } } },
          ...Object.entries(below).map(([type, allowed]) => ({
            from: { element: { type } },
            allow: { to: { element: { types: { anyOf: allowed.length ? allowed : ['none'] } } } },
          })),
        ],
      }],
    },
  }],
});
