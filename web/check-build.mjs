import assert from 'node:assert/strict';
import { readFile, readdir, realpath } from 'node:fs/promises';
import { extname, isAbsolute, relative, resolve, sep } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const repository = fileURLToPath(new URL('..', import.meta.url));
const output = await realpath(resolve(repository, process.env.WEB_OUT || 'dist'));
const files = await readdir(output, { recursive: true, withFileTypes: true });
const paths = files.filter(file => file.isFile()).map(file => resolve(file.parentPath, file.name));
const insideOutput = path => {
  const name = relative(output, path);
  assert(name && !isAbsolute(name) && name !== '..' && !name.startsWith(`..${sep}`), `Asset escapes the build: ${path}`);
};

for (const path of paths) {
  const name = relative(output, path).replaceAll('\\', '/');
  assert(!/(^|\/)(fixtures?|node_modules|pkg)(\/|$)|(?:^|[./-])test(?:[./-]|$)|\.(?:map|d\.ts)$/.test(name), `Development file shipped: ${name}`);
}

const html = await readFile(resolve(output, 'index.html'), 'utf8');
const entries = new Set();
const assets = [...html.matchAll(/<(script|link)\b[^>]*\b(?:src|href)=["']([^"']+)["'][^>]*>/g)];
assert(assets.some(([, tag]) => tag === 'script'), 'The built page has no JavaScript entry.');
assert(assets.some(([element, tag]) => tag === 'link' && /\brel=["']stylesheet["']/.test(element)), 'The built page has no stylesheet.');
for (const [, tag, reference] of assets) {
  assert(!/^(?:\/|[a-z][a-z\d+.-]*:)/i.test(reference), `Asset URL must be relative: ${reference}`);
  for (const prefix of ['/', '/embedded/games/play/']) {
    const url = new URL(reference, `https://build-check.invalid${prefix}index.html`);
    assert(url.pathname.startsWith(prefix), `Asset escaped its deployment prefix: ${reference}`);
    const path = await realpath(resolve(output, decodeURIComponent(url.pathname.slice(prefix.length))));
    insideOutput(path);
    if (tag === 'script') entries.add(path);
  }
}

// Import only non-entry chunks: the page entry itself requires a browser DOM.
// Finding the public runtime exports avoids depending on minified identifiers.
const runtimes = [];
for (const path of paths.filter(path => extname(path) === '.js' && !entries.has(path))) {
  const module = await import(pathToFileURL(path).href);
  if (typeof module.GameFiles === 'function' && typeof module.requires_reload === 'function') runtimes.push(module);
}
assert.equal(runtimes.length, 1, 'Expected one bundled game runtime.');
const runtime = runtimes[0];
const originalFetch = globalThis.fetch;
let wasmRequests = 0;
try {
  globalThis.fetch = async input => {
    const url = new URL(input instanceof Request ? input.url : input);
    assert.equal(url.protocol, 'file:', 'Default WASM URL should be relative to the runtime chunk.');
    const path = await realpath(fileURLToPath(url));
    insideOutput(path);
    assert.equal(extname(path), '.wasm', 'Runtime requested an unexpected asset.');
    wasmRequests++;
    return new Response(await readFile(path), { headers: { 'Content-Type': 'application/wasm' } });
  };
  await runtime.default();
  assert.equal(wasmRequests, 1, 'The default initializer must fetch the emitted WASM.');
  const game = new runtime.GameFiles('Build smoke test');
  try {
    game.add_file('ItemList.bin', new Uint8Array([1, 2, 3]));
    assert.equal(runtime.requires_reload(), false);
  } finally {
    game.free();
  }
} finally {
  globalThis.fetch = originalFetch;
}
console.log('Production bundle passed: relative assets, clean output, and real minified WASM initialization.');
