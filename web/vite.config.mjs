import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';
import { defineConfig } from 'vite';

const root = fileURLToPath(new URL('.', import.meta.url));
const repository = resolve(root, '..');
const target = resolve(repository, process.env.CARGO_TARGET_DIR || 'target');

export default defineConfig({
  root,
  base: './',
  publicDir: false,
  resolve: {
    // Keep generated bindings in Cargo's build directory, but bundle them and
    // their inline JS dependencies just like the rest of the browser code.
    alias: {
      './pkg/nwyd.js': resolve(target, 'web/pkg/nwyd.js'),
    },
  },
  build: {
    outDir: resolve(repository, process.env.WEB_OUT || 'dist'),
    emptyOutDir: true,
    assetsInlineLimit: 0,
    minify: 'oxc',
    cssMinify: 'lightningcss',
    sourcemap: false,
  },
});
