import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import { gestaltsPlugins } from '../../scripts/vite-gestalts-plugins';

export default defineConfig({
  server: { port: 5174 },
  preview: { port: 4174 },
  plugins: [react(), tailwindcss(), gestaltsPlugins()],
  base: './',
});
