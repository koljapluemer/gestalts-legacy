declare module 'virtual:gestalts-plugins' {
  import type { Distribution, PluginEntry } from '@gestalts/ui/host';

  /** this app's `distribution.yml` */
  export const distribution: Distribution;
  /** the plugins it lists, built in (see `scripts/vite-gestalts-plugins.ts`) */
  export const plugins: PluginEntry[];
}
