import { openAssetCache } from './cache.mjs';
import { planFiles } from './files.mjs';

// This is deliberately a browser test: native transaction timing, Blob cloning,
// document lifetimes, and cross-window locks are not modeled by IndexedDB fakes.
const isClient = new URL(location.href).searchParams.has('client');
const timeout = (promise, message, milliseconds = 10_000) => {
  let timer;
  return Promise.race([
    promise,
    new Promise((_, reject) => { timer = setTimeout(() => reject(new Error(message)), milliseconds); }),
  ]).finally(() => clearTimeout(timer));
};
const deferred = () => {
  let resolve;
  const promise = new Promise(done => { resolve = done; });
  return { promise, resolve };
};
const assert = (condition, message) => { if (!condition) throw new Error(message); };
const equal = (actual, expected, message) => assert(JSON.stringify(actual) === JSON.stringify(expected),
  `${message}: expected ${JSON.stringify(expected)}, received ${JSON.stringify(actual)}`);

function makePlan(name, tag) {
  return planFiles(['ItemList.bin', 'UI/Panel.WYS', 'Env/Zone.TRN'].map((path, index) => {
    const file = new File([`${tag}:${index}`], path.split('/').at(-1));
    Object.defineProperty(file, 'webkitRelativePath', { value: `${name}/${path}` });
    return file;
  }));
}

function createGame(root) {
  return {
    root, files: [], freed: false,
    add_file(path, bytes) { this.files.push([path, new TextDecoder().decode(bytes)]); },
    free() { this.freed = true; },
  };
}

function checkGame(game, name, tag) {
  equal(game.root, name, 'Selected root round-trips');
  equal([...game.files].sort(), [
    ['ItemList.bin', `${tag}:0`], ['UI/Panel.WYS', `${tag}:1`], ['Env/Zone.TRN', `${tag}:2`],
  ].sort(), 'Every selected file round-trips without mixing generations');
  assert(!game.freed, 'A successful imported session remains usable');
}

if (isClient) {
  let cache;
  const events = [];
  const locks = {
    request(...args) {
      const callback = args.pop();
      const event = { name: args[0], mode: args[1]?.mode ?? 'exclusive', acquired: false };
      events.push(event);
      return navigator.locks.request(...args, async lock => {
        event.acquired = true;
        return callback(lock);
      });
    },
  };
  window.cacheTestClient = {
    events,
    async open(dbName) { cache = await openAssetCache({ name: dbName, locks }); },
    plan: makePlan,
    getInfo: () => cache.getInfo(),
    save: (plan, progress = () => {}) => cache.save(plan, progress),
    load: (factory = createGame, progress = () => {}, expectedId) => cache.load(factory, progress, expectedId),
    clear: () => cache.clear(),
    close: () => cache?.close(),
  };
  document.querySelector('h1').textContent = 'Cache test client ready';
} else {
  const output = document.querySelector('#results');
  const button = document.querySelector('#run');
  button.addEventListener('click', async () => {
    button.disabled = true;
    output.textContent = '';
    output.className = '';
    delete output.dataset.result;
    const log = text => { output.textContent += `${text}\n`; };
    const dbName = `nwyd-assets-browser-test-${crypto.randomUUID()}`;
    const clients = new Set();
    let passed = 0;
    async function newClient() {
      const frame = document.createElement('iframe');
      const loaded = new Promise((resolve, reject) => {
        frame.onload = resolve;
        frame.onerror = reject;
      });
      frame.src = `./cache.browser.test.html?client=${crypto.randomUUID()}`;
      document.body.append(frame);
      await timeout(loaded, 'Cache test client did not load');
      const client = frame.contentWindow.cacheTestClient;
      assert(client, 'Cache test client module initialized');
      const entry = { frame, client };
      clients.add(entry);
      await client.open(dbName);
      return entry;
    }
    async function removeClient(entry) {
      entry.client.close();
      entry.frame.remove();
      clients.delete(entry);
    }
    async function test(name, run) {
      await timeout(run(), `Timed out: ${name}`);
      passed++;
      log(`PASS ${name}`);
    }
    try {
      assert(isSecureContext && navigator.locks && indexedDB, 'This test needs HTTPS or localhost, IndexedDB, and Web Locks');
      let a = await newClient();
      let b = await newClient();
      const original = a.client.plan('Game', 'original');
      let originalInfo;
      await test('Saved assets are reused by another window and a fresh window', async () => {
        equal(await a.client.getInfo(), null, 'A fresh database has no cache');
        originalInfo = await a.client.save(original);
        equal(originalInfo.count, 3, 'Manifest records the number of files');
        equal(originalInfo.total, original.total, 'Manifest records bytes');
        checkGame((await b.client.load()).game, 'Game', 'original');
        await removeClient(b);
        b = await newClient();
        checkGame((await b.client.load()).game, 'Game', 'original');
      });
      await test('An interrupted replacement preserves the previous complete cache', async () => {
        let interrupted = false;
        try {
          await a.client.save(a.client.plan('Broken', 'incomplete'), () => { throw new Error('Test interrupted import'); });
        } catch (error) {
          assert(error.message.includes('Test interrupted import'), 'The import failure is propagated');
          interrupted = true;
        }
        assert(interrupted, 'The replacement must fail');
        equal((await b.client.getInfo()).id, originalInfo.id, 'Previous manifest is still active');
        checkGame((await b.client.load()).game, 'Game', 'original');
      });
      await test('An active reader finishes its generation before replacement commits', async () => {
        const paused = deferred();
        const resume = deferred();
        let stopped = false;
        const read = a.client.load(createGame, async () => {
          if (stopped) return;
          stopped = true;
          paused.resolve();
          await resume.promise;
        });
        await timeout(paused.promise, 'Reader did not pause');
        const eventsBefore = b.client.events.length;
        const replacing = b.client.save(b.client.plan('Replacement', 'new'));
        try {
          assert(b.client.events.length > eventsBefore, 'Replacement requests a cross-window lock');
          assert(!b.client.events.at(-1).acquired, 'Replacement cannot acquire an exclusive lock during a reader');
        } finally { resume.resolve(); }
        checkGame((await read).game, 'Game', 'original');
        await replacing;
        checkGame((await a.client.load()).game, 'Replacement', 'new');
      });
      await test('Concurrent saves publish one complete generation', async () => {
        const results = await Promise.all([
          a.client.save(a.client.plan('First', 'first')),
          b.client.save(b.client.plan('Second', 'second')),
        ]);
        const { game, info } = await a.client.load();
        assert(results.some(result => result.id === info.id), 'Active generation belongs to a completed import');
        checkGame(game, info.root, info.root === 'First' ? 'first' : 'second');
      });
      await test('A stale launch selection cannot silently load a replacement', async () => {
        let rejected = false;
        let constructed = false;
        try {
          await a.client.load(root => { constructed = true; return createGame(root); }, () => {}, originalInfo.id);
        } catch (error) {
          rejected = error.code === 'changed';
        }
        assert(rejected, 'Stale metadata is rejected with a changed error');
        assert(!constructed, 'A stale selection is rejected before creating a game');
      });
      await test('Failed imports free partially constructed WASM game objects', async () => {
        let partial;
        let rejected = false;
        try {
          await a.client.load(root => (partial = createGame(root)), () => { throw new Error('Test loading cancelled'); });
        } catch (error) {
          rejected = error.message.includes('Test loading cancelled');
        }
        assert(rejected, 'Loading failure is propagated');
        assert(partial?.freed, 'Partial game allocations are released');
        assert(partial.files.length > 0, 'The failure happened after some bytes were imported');
      });
      await test('Clearing cached storage preserves assets already loaded by a session', async () => {
        const { game, info } = await a.client.load();
        await b.client.clear();
        equal(await a.client.getInfo(), null, 'Another window observes cleared metadata');
        equal(await a.client.load(), null, 'Launching requires a new import after clearing');
        checkGame(game, info.root, info.root === 'First' ? 'first' : 'second');
      });
      await test('Closing an importing window leaves the previous cache recoverable', async () => {
        await a.client.save(original);
        const paused = deferred();
        // A never-resolved progress callback mimics a closed page during import.
        // Removing the frame releases the browser-owned lock and IDB connection.
        void b.client.save(b.client.plan('Closed', 'partial'), () => {
          paused.resolve();
          return new Promise(() => {});
        }).catch(() => {});
        await timeout(paused.promise, 'Closing-window importer did not pause');
        b.frame.remove();
        clients.delete(b);
        b = await newClient();
        checkGame((await b.client.load()).game, 'Game', 'original');
        await b.client.save(b.client.plan('Recovered', 'recovered'));
        checkGame((await a.client.load()).game, 'Recovered', 'recovered');
      });
      log(`\n${passed} browser integration tests passed.`);
      output.className = 'pass';
      output.dataset.result = 'passed';
    } catch (error) {
      log(`FAIL ${error.stack ?? error}`);
      output.className = 'fail';
      output.dataset.result = 'failed';
    } finally {
      for (const entry of clients) await removeClient(entry);
      await timeout(new Promise((resolve, reject) => {
        const request = indexedDB.deleteDatabase(dbName);
        request.onsuccess = resolve;
        request.onerror = () => reject(request.error);
      }), 'Test database cleanup was blocked').catch(error => log(`Cleanup: ${error.message}`));
      button.disabled = false;
    }
  });
}
