import test from 'node:test';
import assert from 'node:assert/strict';
import { IDBFactory } from 'fake-indexeddb';
import { openAssetCache } from './cache.mjs';
import { planFiles } from './files.mjs';

const deferred = () => {
  let resolve;
  const promise = new Promise(done => { resolve = done; });
  return { promise, resolve };
};

// Models the Web Locks queue for IndexedDB unit tests. The browser harness also
// exercises actual origin-wide locks with independently destroyed iframe tabs.
class Locks {
  constructor() { this.names = new Map(); }
  request(name, { mode }, action) {
    const state = this.names.get(name) ?? { active: 0, writer: false, queue: [] };
    this.names.set(name, state);
    return new Promise((resolve, reject) => {
      state.queue.push({ mode, action, resolve, reject });
      this.pump(state);
    });
  }
  pump(state) {
    while (state.queue.length && !state.writer) {
      const first = state.queue[0];
      if (first.mode === 'exclusive' && state.active) break;
      state.queue.shift();
      state.active++;
      state.writer = first.mode === 'exclusive';
      Promise.resolve().then(first.action).then(first.resolve, first.reject).finally(() => {
        state.active--;
        state.writer = false;
        this.pump(state);
      });
    }
  }
}

function plan(root = 'Game', contents = ['items', 'panel', 'terrain']) {
  return planFiles(['ItemList.bin', 'UI/Panel.WYS', 'Env/Zone.TRN'].map((path, index) => {
    const file = new Blob([contents[index]]);
    file.webkitRelativePath = `${root}/${path}`;
    return file;
  }));
}

function receiver() {
  return { files: [], freed: false,
    add_file(path, bytes) { this.files.push([path, new TextDecoder().decode(bytes)]); },
    free() { this.freed = true; },
  };
}

async function caches(t, count = 1) {
  const indexedDB = new IDBFactory();
  const locks = new Locks();
  const instances = await Promise.all(Array.from({ length: count }, () => openAssetCache({ indexedDB, locks })));
  t.after(() => instances.forEach(cache => cache.close()));
  return instances;
}

function edit(cache, store, operation) {
  return new Promise((resolve, reject) => {
    const tx = cache.db.transaction(store, 'readwrite');
    tx.oncomplete = resolve;
    tx.onabort = () => reject(tx.error);
    operation(tx.objectStore(store));
  });
}

function entries(cache, store) {
  return new Promise((resolve, reject) => {
    const tx = cache.db.transaction(store);
    const request = tx.objectStore(store).getAll();
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

test('unavailable storage or Web Locks leaves direct folder imports possible', async () => {
  await assert.rejects(openAssetCache({ indexedDB: null, locks: new Locks() }), { code: 'unavailable' });
  await assert.rejects(openAssetCache({ indexedDB: new IDBFactory(), locks: null }), { code: 'unavailable' });
});

test('cached Blob bytes, source ordering, root and progress survive another connection', async t => {
  const [writer, reader] = await caches(t, 2);
  assert.equal(await reader.getInfo(), null);
  assert.equal(await reader.load(() => assert.fail('empty cache creates no game')), null);
  const source = plan('My installation');
  source.selected.forEach(({ file }) => {
    file.arrayBuffer = () => assert.fail('saving must not materialize the entire file in JS');
  });
  const writes = [];
  const info = await writer.save(source, (...args) => writes.push(args));
  assert.deepEqual(await reader.getInfo(), info);
  assert.equal(info.count, 3);
  assert.equal(info.total, 17);
  const reads = [];
  const loaded = await reader.load((root, metadata) => {
    assert.equal(root, 'My installation');
    assert.deepEqual(metadata, info);
    return receiver();
  }, (...args) => reads.push(args), info.id);
  assert.deepEqual(loaded.game.files, [['ItemList.bin', 'items'], ['UI/Panel.WYS', 'panel'], ['Env/Zone.TRN', 'terrain']]);
  assert.deepEqual(reads, writes);
  assert.deepEqual(reads.map(([size]) => size), [5, 10, 17]);
  writer.close();
  assert.deepEqual(await reader.getInfo(), info);
});

test('interrupted replacements preserve the complete previous generation', async t => {
  const [cache] = await caches(t);
  const previous = await cache.save(plan());
  const stopped = new Error('file selection became unavailable');
  await assert.rejects(cache.save(plan('Replacement'), () => { throw stopped; }), error => error === stopped);
  assert.deepEqual(await cache.getInfo(), previous);
  const loaded = await cache.load(receiver);
  assert.equal(loaded.game.files.length, 3);
  assert.equal((await entries(cache, 'files')).length, 3);
});

test('quota errors and aborted commits cannot publish a partial installation', async t => {
  const [cache] = await caches(t);
  const previous = await cache.save(plan());
  const original = cache.db.transaction.bind(cache.db);
  for (const failure of ['quota', 'abort-after-put']) {
    let injected = false;
    cache.db.transaction = (...args) => {
      const tx = original(...args);
      const objectStore = tx.objectStore.bind(tx);
      tx.objectStore = name => {
        const store = objectStore(name);
        const put = store.put.bind(store);
        store.put = (...putArgs) => {
          if (name === 'files' && !injected) {
            injected = true;
            if (failure === 'quota') throw new DOMException('Storage quota exhausted', 'QuotaExceededError');
            const request = put(...putArgs);
            request.addEventListener('success', () => tx.abort());
            return request;
          }
          return put(...putArgs);
        };
        return store;
      };
      return tx;
    };
    await assert.rejects(cache.save(plan('Replacement')), failure === 'quota' ? { name: 'QuotaExceededError' } : { code: 'aborted' });
    cache.db.transaction = original;
    assert.equal(injected, true);
    assert.deepEqual(await cache.getInfo(), previous);
    assert.equal((await entries(cache, 'files')).length, 3);
  }
});

test('the next save collects a terminated writer and the old committed generation', async t => {
  const [cache] = await caches(t);
  const previous = await cache.save(plan());
  await edit(cache, 'files', store => store.put({ generation: 'terminated-tab', order: 0, path: 'ItemList.bin', size: 4, blob: new Blob(['part']) }));
  const next = await cache.save(plan('Replacement'));
  assert.notEqual(previous.id, next.id);
  assert.equal(next.root, 'Replacement');
  assert.deepEqual(new Set((await entries(cache, 'files')).map(entry => entry.generation)), new Set([next.id]));
});

test('an active reader retains one generation while another tab replaces it', async t => {
  const [reader, writer] = await caches(t, 2);
  const previous = await writer.save(plan('Original'));
  const reading = deferred();
  const release = deferred();
  let received = 0;
  let writing = false;
  const load = reader.load(receiver, async () => {
    if (received++ === 0) { reading.resolve(); await release.promise; }
  }, previous.id);
  await reading.promise;
  const save = writer.save(plan('Replacement'), () => { writing = true; });
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(writing, false);
  release.resolve();
  const loaded = await load;
  const next = await save;
  assert.equal(loaded.info.root, 'Original');
  assert.equal(loaded.game.files.length, 3);
  assert.equal(next.root, 'Replacement');
  await assert.rejects(reader.load(() => assert.fail('stale selection creates no game'), undefined, previous.id), { code: 'changed' });
});

test('clear waits for readers and is atomic across metadata and blobs', async t => {
  const [reader, writer] = await caches(t, 2);
  await writer.save(plan());
  const reading = deferred();
  const release = deferred();
  const load = reader.load(receiver, async () => { reading.resolve(); await release.promise; });
  await reading.promise;
  let cleared = false;
  const clearing = writer.clear().then(() => { cleared = true; });
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(cleared, false);
  release.resolve();
  assert.equal((await load).game.files.length, 3);
  await clearing;
  assert.equal(await reader.getInfo(), null);
  assert.deepEqual(await entries(writer, 'files'), []);
});

test('concurrent writers serialize and publish complete installations only', async t => {
  const [first, second] = await caches(t, 2);
  const started = deferred();
  const release = deferred();
  let firstFile = true;
  const savingFirst = first.save(plan('First'), async () => {
    if (firstFile) { firstFile = false; started.resolve(); await release.promise; }
  });
  await started.promise;
  let secondStarted = false;
  const savingSecond = second.save(plan('Second'), () => { secondStarted = true; });
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(secondStarted, false);
  release.resolve();
  const [one, two] = await Promise.all([savingFirst, savingSecond]);
  assert.notEqual(one.id, two.id);
  assert.deepEqual(await first.getInfo(), two);
  assert.deepEqual(new Set((await entries(first, 'files')).map(entry => entry.generation)), new Set([two.id]));
});

test('missing or corrupt blobs free partial WASM owners and can be replaced', async t => {
  const [cache] = await caches(t);
  for (const damage of ['missing', 'size', 'path', 'blob']) {
    const info = await cache.save(plan());
    await edit(cache, 'files', store => {
      if (damage === 'missing') return store.delete([info.id, 1]);
      const request = store.get([info.id, 1]);
      request.onsuccess = () => {
        const record = request.result;
        if (damage === 'size') record.size++;
        if (damage === 'path') record.path = '../outside';
        if (damage === 'blob') record.blob = 'not a Blob';
        store.put(record);
      };
    });
    const game = receiver();
    await assert.rejects(cache.load(() => game), { code: 'corrupt' });
    assert.equal(game.freed, true);
    assert.equal(game.files.length, 1);
  }
  const replacement = await cache.save(plan('Fixed'));
  assert.deepEqual((await cache.load(receiver)).info, replacement);
});

test('metadata corruption is rejected before game creation and clear repairs the cache', async t => {
  const [cache] = await caches(t);
  const info = await cache.save(plan());
  await edit(cache, 'meta', store => store.put({ ...info, formatVersion: 99 }, 'active'));
  await assert.rejects(cache.getInfo(), { code: 'corrupt' });
  await assert.rejects(cache.load(() => assert.fail('corrupt metadata creates no game')), { code: 'corrupt' });
  await cache.clear();
  assert.equal(await cache.getInfo(), null);
});

test('WASM rejection frees its partial asset collection and preserves cache data', async t => {
  const [cache] = await caches(t);
  const info = await cache.save(plan());
  const exhausted = new Error('wasm memory exhausted');
  const game = receiver();
  game.add_file = () => { throw exhausted; };
  await assert.rejects(cache.load(() => game), error => error === exhausted);
  assert.equal(game.freed, true);
  assert.deepEqual(await cache.getInfo(), info);
});

test('invalid source plans fail before modifying a ready cache', async t => {
  const [cache] = await caches(t);
  const info = await cache.save(plan());
  for (const change of [
    source => { source.total++; },
    source => { source.root = '../Game'; },
    source => { source.selected[1].path = '/absolute'; },
    source => { source.selected[1].path = 'ItemList.bin'; },
    source => { source.selected[1].file = {}; },
    source => { source.selected[1].path = 'Other/Panel.WYS'; },
  ]) {
    const source = plan();
    change(source);
    await assert.rejects(cache.save(source), { code: 'corrupt' });
    assert.deepEqual(await cache.getInfo(), info);
  }
});

test('a closed connection fails clearly without deleting the saved installation', async t => {
  const [closed, other] = await caches(t, 2);
  const info = await closed.save(plan());
  closed.close();
  await assert.rejects(closed.getInfo(), { code: 'closed' });
  assert.deepEqual(await other.getInfo(), info);
});
