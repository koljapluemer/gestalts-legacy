import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import { gestaltsPlugins } from '../../scripts/vite-gestalts-plugins';

export default defineConfig({
  server: { port: 5173 },
  preview: { port: 4173 },
  plugins: [react(), tailwindcss(), gestaltsPlugins()],
  base: './',
});
