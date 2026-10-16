import { openAssetCache } from './cache.mjs';
import { planFiles } from './files.mjs';

const results = document.querySelector('#results');
const summary = document.querySelector('#summary');
window.launcherTestResults = { complete: false, passed: 0, failed: 0, failures: [] };

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

async function until(predicate, message, timeout = 10000) {
  const deadline = performance.now() + timeout;
  do {
    const value = await predicate();
    if (value) return value;
    await new Promise(resolve => setTimeout(resolve, 20));
  } while (performance.now() < deadline);
  throw new Error(`Timed out: ${message}`);
}

function visible(element) {
  return element && !element.closest('[hidden]');
}

function plan(root, marker) {
  return planFiles(['ItemList.bin', 'UI/Panel.WYS', 'Env/Zone.TRN'].map(path => {
    const file = new File([new Uint8Array([marker])], path.split('/').at(-1));
    Object.defineProperty(file, 'webkitRelativePath', { value: `${root}/${path}` });
    return file;
  }));
}

async function context() {
  const name = `nwyd-launcher-test-${crypto.randomUUID()}`;
  const cache = await openAssetCache({ name });
  const frames = [];
  return {
    cache,
    async frame(options = {}, whileLoading) {
      const url = new URL('./fixtures/launcher-frame.html', import.meta.url);
      url.searchParams.set('cacheName', name);
      for (const [key, value] of Object.entries(options)) url.searchParams.set(key, value);
      const element = document.createElement('iframe');
      element.title = `Independent launcher ${frames.length + 1}`;
      element.src = url;
      document.querySelector('#frames').append(element);
      frames.push(element);
      if (whileLoading) {
        const fixture = await until(() => element.contentWindow?.__launcherTest, 'launcher fixture');
        await whileLoading(fixture);
      }
      await until(() => element.contentWindow?.__launcherTest?.ready, 'launcher initialization');
      const fixture = element.contentWindow.__launcherTest;
      assert(!fixture.error, fixture.error);
      await until(() => !element.contentDocument.querySelector('input[type=file]').disabled ||
        fixture.runtime.starts.length || fixture.runtime.releaseRead || fixture.runtime.releaseHealth ||
        element.contentDocument.querySelector('.status.error'), 'initial cache lookup');
      return {
        fixture,
        runtime: fixture.runtime,
        query: selector => element.contentDocument.querySelector(selector),
        queries: selector => Array.from(element.contentDocument.querySelectorAll(selector)),
        choose: (...args) => fixture.chooseFolder(...args),
        async launch() {
          const button = await until(() => {
            const button = element.contentDocument.querySelector('.launch');
            return visible(button) && !button.disabled ? button : null;
          }, 'enabled manual launch button');
          button.click();
        },
        selectRelay(mode) {
          const select = element.contentDocument.querySelector('.relay-mode');
          select.value = mode;
          select.dispatchEvent(new Event('change', { bubbles: true }));
        },
        detectRelay: () => fixture.focusRelayCheck(),
      };
    },
    async dispose() {
      for (const frame of frames) {
        frame.contentWindow.__launcherTest?.controller?.dispose();
        frame.remove();
      }
      cache.close();
      await new Promise((resolve, reject) => {
        const request = indexedDB.deleteDatabase(name);
        request.onsuccess = resolve;
        request.onerror = () => reject(request.error);
        request.onblocked = () => reject(new Error(`Test database still in use: ${name}`));
      });
    },
  };
}

async function test(name, run) {
  const item = document.createElement('li');
  item.textContent = `${name} — running…`;
  results.append(item);
  let testContext;
  try {
    testContext = await context();
    await run(testContext);
    await testContext.dispose();
    testContext = null;
    window.launcherTestResults.passed++;
    item.className = 'pass';
    item.textContent = `${name} — passed`;
  } catch (error) {
    window.launcherTestResults.failed++;
    window.launcherTestResults.failures.push({ name, error: error?.stack ?? String(error) });
    item.className = 'fail';
    item.textContent = `${name} — ${error?.stack ?? String(error)}`;
  } finally {
    if (testContext) await testContext.dispose().catch(() => {});
  }
}

await test('Selecting a folder saves immediately, while new and returning players launch explicitly', async ({ cache, frame }) => {
  const first = await frame({ holdStart: '1' });
  assert(!first.query('input[type=file]').disabled, 'Folder selection should be enabled.');
  assert(visible(first.query('.folder-selection')), 'A page without saved files should offer folder selection.');
  assert(!first.query('.play-cache, .clear-cache, .new-session, .connect-relay'), 'The launcher should not require session-management or relay-connection controls.');
  assert(!first.query('.launcher-header, .launcher-footer, .launcher-intro'), 'The launcher should omit promotional sections.');
  assert(first.query('.relay-mode'), 'Players should be able to choose their relay.');
  first.choose('First Game', 17);
  assert(first.runtime.starts.length === 0, 'Selecting files must not launch automatically.');
  await first.launch();
  await until(() => first.runtime.starts.length === 1, 'initial WASM start');
  const info = await until(() => cache.getInfo(), 'cache save before successful startup');
  assert(info.root === 'First Game' && info.count === 3 && info.total === 3, 'The stored folder summary should match the imported files.');
  first.runtime.releaseStart();
  await until(() => first.query('.nwyd-launcher').hidden, 'successful startup');
  assert(first.query('.nwyd-launcher').hidden && !first.query('.nwyd-player').hidden, 'Successful startup should reveal the player.');
  assert(first.query('input[type=file]').files.length === 0, 'The launcher should release selected File references.');

  const second = await frame();
  assert(second.runtime.starts.length === 0, 'Returning players must explicitly launch.');
  await second.launch();
  await until(() => second.runtime.completed === 1, 'explicit cached startup');
  assert(second.runtime.starts[0].root === 'First Game', 'Cached startup should use the saved root.');
  assert(second.runtime.starts[0].files.every(file => file.bytes[0] === 17), 'Cached startup should import the saved bytes.');
  assert(second.query('input[type=file]').files.length === 0, 'A returning player must not need a folder selection.');
  assert((await cache.getInfo()).id === info.id, 'Playing cached files should not rewrite the cache.');

  const third = await frame();
  await third.launch();
  await until(() => third.runtime.completed === 1, 'explicit new-session startup');
  assert(third.runtime.starts[0].files.every(file => file.bytes[0] === 17), 'Multiple sessions should import the saved generation independently.');
});

await test('Selected files can be replaced before launch, while canceling or rejecting a folder preserves the valid selection', async ({ cache, frame }) => {
  const selecting = await frame();
  selecting.choose('Initial Selection', 91);
  await until(async () => (await cache.getInfo())?.root === 'Initial Selection', 'initial selection saved');
  assert(visible(selecting.query('.change-folder')), 'A selected folder must offer a replacement action.');
  selecting.query('.change-folder').click();
  assert(visible(selecting.query('.folder-selection')), 'Changing the folder must reveal the picker.');
  selecting.fixture.cancelFolder();
  assert(selecting.query('.file-summary').textContent.includes('Initial Selection'), 'Canceling the picker must retain selected files.');
  selecting.choose('Incomplete Replacement', 0, true);
  await until(() => selecting.query('.status').classList.contains('error'), 'invalid replacement diagnostic');
  assert(selecting.query('.file-summary').textContent.includes('Initial Selection'), 'An invalid replacement must retain the valid folder summary.');
  assert((await cache.getInfo()).root === 'Initial Selection', 'An invalid folder must not replace the saved installation.');
  selecting.choose('Final Selection', 92);
  await until(async () => (await cache.getInfo())?.root === 'Final Selection', 'replacement saved');
  await new Promise(resolve => setTimeout(resolve, 50));
  selecting.detectRelay();
  await new Promise(resolve => setTimeout(resolve, 30));
  assert(selecting.runtime.starts.length === 0, 'Choosing a replacement and detecting the relay must still wait for an explicit launch.');
  await selecting.launch();
  await until(() => selecting.runtime.completed === 1, 'selected replacement launched');
  assert(selecting.runtime.starts[0].root === 'Final Selection' && selecting.runtime.starts[0].files.every(file => file.bytes[0] === 92), 'The runtime must receive only the replacement files.');
});

await test('A cached folder can be replaced before launching without discarding it on an invalid choice', async ({ cache, frame }) => {
  const saved = await cache.save(plan('Cached Installation', 93));
  const selecting = await frame();
  assert(visible(selecting.query('.change-folder')), 'A cached folder must offer a replacement action.');
  selecting.query('.change-folder').click();
  selecting.choose('Invalid Installation', 0, true);
  await until(() => selecting.query('.status').classList.contains('error'), 'invalid cached replacement diagnostic');
  assert((await cache.getInfo()).id === saved.id, 'Invalid replacements must leave the cached generation intact.');
  assert(selecting.query('.file-summary').textContent.includes('Cached Installation'), 'Invalid replacements must retain the cached folder summary.');
  selecting.choose('Selected Replacement', 94);
  await selecting.launch();
  await until(() => selecting.runtime.completed === 1, 'replacement of cached files launched');
  assert(selecting.runtime.starts[0].root === 'Selected Replacement', 'Explicit launch must use the new selection instead of the previous cached generation.');
  assert(selecting.runtime.starts[0].files.every(file => file.bytes[0] === 94), 'The replacement bytes must reach the runtime.');
});

await test('Replacing a selection during a pending save publishes only the latest complete folder', async ({ cache, frame }) => {
  const selecting = await frame({ holdSave: '1' });
  selecting.choose('Superseded Selection', 95);
  await until(() => selecting.runtime.releaseSave, 'first save held');
  selecting.query('.change-folder').click();
  selecting.choose('Current Selection', 96);
  selecting.runtime.releaseSave();
  await until(async () => (await cache.getInfo())?.root === 'Current Selection', 'current selection published');
  await selecting.launch();
  await until(() => selecting.runtime.completed === 1, 'latest selected files launched');
  assert(selecting.runtime.starts[0].root === 'Current Selection' && selecting.runtime.starts[0].files.every(file => file.bytes[0] === 96), 'A superseded save must never replace the current selection or running files.');
});

await test('Local relay selection works when the remote relay is unavailable and persists for the next page', async ({ cache, frame }) => {
  await cache.save(plan('Local Relay Game', 97));
  const local = await frame({ relayMissing: '1' });
  await until(() => /Não foi possível conectar/.test(local.query('.relay-status').textContent), 'unavailable remote relay');
  assert(local.runtime.loopbackRequests === 0, 'Default remote mode must not probe localhost.');
  local.selectRelay('local');
  await until(() => local.runtime.loopbackRequests > 0, 'local relay discovery after explicit selection');
  const instructions = local.queries('a').find(anchor => anchor.href === 'https://github.com/notwyd/nwyd/releases/latest');
  assert(visible(instructions), 'Local mode must show a link to the downloadable relay release.');
  assert(local.query('.relay-mode').value === 'local', 'The launcher must retain the chosen local mode.');
  assert(local.runtime.starts.length === 0, 'Choosing a relay must not launch the game automatically.');
  await local.launch();
  await until(() => local.runtime.completed === 1, 'local relay launch');
  assert(new URL(local.runtime.starts[0].gateway).origin === 'ws://127.0.0.1:8787', 'Local play must connect its game socket to the local relay.');
  assert(new URL(local.runtime.starts[0].httpProxy).origin === 'http://127.0.0.1:8787', 'Local HTTP traffic must use the local relay.');
  assert(new URL(local.runtime.starts[0].gateway).searchParams.get('token') === 'c'.repeat(64), 'Local play must use the token from the selected relay.');
  const next = await frame();
  await until(() => next.runtime.healthRequests > 0, 'restored relay preference');
  assert(next.query('.relay-mode').value === 'local', 'The relay preference must survive another page load in the same browser session.');
  assert(next.runtime.healthUrls.every(url => url === 'http://127.0.0.1:8787/health'), 'Restoring local mode must not probe the remote relay first.');
  assert(next.runtime.starts.length === 0, 'Restoring the mode must still require an explicit first launch.');
});

await test('Switching relays cancels an outstanding probe and ignores its late response', async ({ cache, frame }) => {
  await cache.save(plan('Switched Relay Game', 98));
  const selecting = await frame({ holdHealth: '1', ignoreHealthAbort: '1' });
  const releaseRemote = await until(() => selecting.runtime.releaseHealth, 'held remote health request');
  selecting.selectRelay('local');
  await until(() => selecting.runtime.loopbackRequests === 1 && !selecting.query('.launch').disabled, 'new local relay connection');
  assert(selecting.runtime.healthAborts === 1, 'Changing the relay must abort the previous network request.');
  releaseRemote();
  await new Promise(resolve => setTimeout(resolve, 30));
  assert(selecting.query('.relay-mode').value === 'local', 'A late remote response must not change the chosen mode.');
  await selecting.launch();
  await until(() => selecting.runtime.completed === 1, 'launch after stale remote health response');
  assert(new URL(selecting.runtime.starts[0].gateway).origin === 'ws://127.0.0.1:8787', 'A late remote response must not overwrite the active connection.');
  assert(!/cancelad|incompatível/.test(selecting.query('.relay-status').textContent), 'Cancellation of the previous probe must not overwrite the new relay status with an error.');
});

await test('Switching away from an unavailable local relay preserves selected files and does not silently fall back', async ({ frame }) => {
  const selecting = await frame({ localRelayMissing: '1' });
  selecting.selectRelay('local');
  selecting.choose('Relay Choice Preserves Files', 99);
  await until(() => selecting.query('.relay-status').classList.contains('error'), 'unavailable local relay diagnostic');
  const remoteRequests = selecting.runtime.healthUrls.filter(url => url.startsWith('https:')).length;
  selecting.detectRelay();
  await new Promise(resolve => setTimeout(resolve, 40));
  assert(selecting.runtime.healthUrls.filter(url => url.startsWith('https:')).length === remoteRequests, 'Local connection failures must not silently probe the remote relay.');
  assert(selecting.runtime.starts.length === 0, 'An unavailable selected relay must block launching.');
  selecting.selectRelay('remote');
  await selecting.launch();
  await until(() => selecting.runtime.completed === 1, 'manual remote launch after mode change');
  assert(selecting.runtime.starts[0].root === 'Relay Choice Preserves Files' && selecting.runtime.starts[0].files.every(file => file.bytes[0] === 99), 'Changing the relay must keep the selected files.');
  assert(new URL(selecting.runtime.starts[0].gateway).origin === 'wss://relay.notwyd.app', 'Explicitly switching back must restore the remote game endpoint.');
});

await test('A runtime failure does not prevent structurally valid selected files from being shared with the editor', async ({ cache, frame }) => {
  const first = await frame({ failStart: '1' });
  first.choose('Rejected First Game', 21);
  await first.launch();
  await until(() => /Deliberate test runtime startup failure/.test(first.query('.status').textContent), 'failed first launch');
  const firstSaved = await until(() => cache.getInfo(), 'selected folder saved despite failed runtime');
  assert(firstSaved.root === 'Rejected First Game', 'Sharing files must not depend on a playable session.');
  assert(first.query('.nwyd-player').hidden, 'A failed runtime must return to the launcher.');

  await cache.save(plan('Known Good Game', 22));
  const replacement = await frame({ failStart: '1' });
  await replacement.launch();
  await until(() => replacement.runtime.starts.length === 1 && !replacement.query('input[type=file]').disabled, 'failed cached startup returning to folder selection');
  replacement.choose('Rejected Replacement', 23);
  await replacement.launch();
  await until(() => replacement.runtime.starts.length === 2 && !replacement.query('input[type=file]').disabled, 'failed replacement launch');
  assert(/Deliberate test runtime startup failure/.test(replacement.query('.status').textContent), 'The runtime error should remain visible.');
  const after = await until(async () => {
    const saved = await cache.getInfo();
    return saved?.root === 'Rejected Replacement' && saved;
  }, 'replacement selected folder saved');
  assert(after.count === 3, 'The replacement must still publish a complete cache.');
});

await test('Unavailable browser storage still permits direct folder play', async ({ cache, frame }) => {
  const direct = await frame({ storageUnavailable: '1' });
  assert(!direct.query('input[type=file]').disabled, 'Storage failure must not disable folder selection.');
  direct.choose('Uncached Game', 31);
  await direct.launch();
  await until(() => direct.runtime.completed === 1, 'direct startup without IndexedDB');
  await until(() => direct.query('.nwyd-launcher').hidden, 'direct player visibility');
  assert(!direct.query('.nwyd-player').hidden, 'Direct startup should reveal the player.');
  assert(await cache.getInfo() === null, 'Direct fallback must not claim a stored snapshot.');
  assert(/armazenamento|pasta/i.test(direct.query('.session-status').textContent), 'The session should explain why files need selecting again in Portuguese.');
});

await test('A cache quota failure preserves the previous installation while the new session keeps playing', async ({ cache, frame }) => {
  const direct = await frame({ quotaOnSave: '1' });
  // A different document finishes saving after this page's initial cache check.
  const original = await cache.save(plan('Saved Before Quota', 35));
  direct.choose('Running Despite Quota', 36);
  await direct.launch();
  await until(() => /não puderam ser salvos/.test(direct.query('.session-status').textContent), 'cache quota warning');
  await until(() => direct.runtime.completed === 1, 'direct startup despite quota failure');
  assert(direct.runtime.completed === 1 && !direct.query('.nwyd-player').hidden, 'Cache write failure must not stop the running game.');
  assert((await cache.getInfo()).id === original.id, 'Quota failure must leave the older cache intact.');
  direct.fixture.closeGame();
  await until(() => !direct.query('.restart').disabled, 'restart available after failed save');
  direct.query('.restart').click();
  assert(direct.runtime.reloads === 1, 'The closed uncached session should still allow returning to a fresh launcher.');
  const next = await frame();
  await new Promise(resolve => setTimeout(resolve, 50));
  assert(next.runtime.starts.length === 0, 'Restart must not silently launch an older cached installation after saving the selected one failed.');
  assert(next.query('.file-summary').textContent.includes('Saved Before Quota'), 'The fresh launcher should identify the older saved folder for an explicit choice.');
  assert(visible(next.query('.launch')) && visible(next.query('.change-folder')), 'The player must be able to explicitly use the saved folder or choose the intended installation.');
});

await test('Replacing or removing stored files affects future page loads and leaves running sessions unchanged', async ({ cache, frame }) => {
  await cache.save(plan('Original Game', 41));
  const running = await frame();
  await running.launch();
  await until(() => running.runtime.completed === 1, 'manual runtime startup');
  await until(() => running.runtime.completed === 1, 'original cached session');
  await cache.save(plan('Replacement Game', 42));
  const replacing = await frame();
  await replacing.launch();
  await until(() => replacing.runtime.completed === 1, 'replacement cached session');
  assert(replacing.runtime.starts[0].root === 'Replacement Game', 'New page loads should use the latest complete cache.');
  assert(replacing.runtime.starts[0].files.every(file => file.bytes[0] === 42), 'New sessions should import the replacement bytes.');
  assert(running.runtime.starts[0].files.every(file => file.bytes[0] === 41), 'An active session should retain its original assets.');
  assert(running.runtime.completed === 1 && !running.query('.nwyd-player').hidden, 'Replacing stored files must leave existing sessions running.');

  await cache.clear();
  const empty = await frame();
  assert(empty.runtime.starts.length === 0, 'A page with no saved files should not start a runtime.');
  assert(visible(empty.query('.folder-selection')) && !empty.query('input[type=file]').disabled, 'A new page should request the folder after cache removal.');
  assert(running.runtime.completed === 1 && !running.query('.nwyd-player').hidden, 'Removing stored files must leave the original session running.');
  assert(replacing.runtime.completed === 1 && !replacing.query('.nwyd-player').hidden, 'Clearing stored files must leave the current session running.');
});

await test('Restart waits for an explicit folder choice when another page replaced the running installation', async ({ cache, frame }) => {
  await cache.save(plan('Running Installation', 43));
  const running = await frame();
  await running.launch();
  await until(() => running.runtime.completed === 1, 'original game running');
  await cache.save(plan('Editor Replacement', 44));
  running.fixture.closeGame();
  running.query('.restart').click();
  const next = await frame();
  await new Promise(resolve => setTimeout(resolve, 50));
  assert(next.runtime.starts.length === 0, 'Restarting a previous session must not silently start an installation supplied by another page.');
  assert(next.query('.file-summary').textContent.includes('Editor Replacement'), 'The launcher should clearly identify the replacement before the player chooses it.');
  await next.launch();
  await until(() => next.runtime.completed === 1, 'replacement explicitly launched');
  assert(next.runtime.starts[0].root === 'Editor Replacement' && next.runtime.starts[0].files.every(file => file.bytes[0] === 44), 'An explicit launch should use the complete current installation.');
});

await test('Disposing during local or cached file loading frees the owner and prevents a late runtime startup', async ({ cache, frame }) => {
  const local = await frame({ holdRead: '1' });
  local.choose('Disposed Local Game', 51);
  await local.launch();
  await until(() => local.runtime.releaseRead, 'paused local file read');
  local.fixture.controller.dispose();
  local.runtime.releaseRead();
  await until(() => local.runtime.freed === 1, 'local GameFiles owner release');
  assert(local.runtime.starts.length === 0, 'Disposal during local import must prevent late startup.');
  const completed = await cache.getInfo();
  assert(!completed || (completed.root === 'Disposed Local Game' && completed.count === 3), 'Disposal may preserve a completed save but must never publish a partial installation.');

  const saved = await cache.save(plan('Disposed Cached Game', 52));
  const cached = await frame({ holdRead: '1' });
  await cached.launch();
  await until(() => cached.runtime.releaseRead, 'paused cached file read');
  cached.fixture.controller.dispose();
  cached.runtime.releaseRead();
  await until(() => cached.runtime.freed === 1, 'cached GameFiles owner release');
  assert(cached.runtime.starts.length === 0, 'Disposal during cached import must prevent late startup.');
  assert((await cache.getInfo()).id === saved.id, 'Disposal should preserve the existing saved installation.');
});

await test('The remote connection retries automatically and selected files survive until it is ready', async ({ cache, frame }) => {
  const first = await frame({ relayMissing: '1' });
  await until(() => /Não foi possível conectar/.test(first.query('.relay-status').textContent), 'automatic remote discovery');
  assert(first.runtime.healthRequests === 1, 'The page should start one remote health request automatically.');
  assert(first.runtime.permissionQueries === 0 && first.runtime.loopbackRequests === 0, 'The remote connection must never request local access.');
  assert(!first.query('.connect-relay'), 'Relay detection must not require a connect button.');
  first.choose('Waiting For Relay', 61);
  assert(first.runtime.created === 0 && first.runtime.starts.length === 0, 'Selecting files must wait for relay readiness before creating WASM owners.');
  const shared = await until(() => cache.getInfo(), 'selected folder saved while waiting for relay');
  assert(shared.root === 'Waiting For Relay', 'The editor must be able to reuse the folder before the relay is ready.');
  first.runtime.relayAvailable = true;
  first.runtime.holdNextHealth = true;
  const requestsBeforeDetection = first.runtime.healthRequests;
  first.detectRelay();
  first.detectRelay();
  await until(() => first.runtime.releaseHealth, 'held automatic relay discovery');
  assert(first.runtime.healthRequests === requestsBeforeDetection + 1, 'Overlapping detection events must share one pending health request.');
  assert(first.runtime.created === 0, 'Held relay discovery must not create a game runtime.');
  first.runtime.releaseHealth();
  await first.launch();
  await until(() => first.runtime.completed === 1, 'startup after automatic relay detection');
  const saved = await until(() => cache.getInfo(), 'selected files saved after relay connection');
  assert(saved.root === 'Waiting For Relay', 'The original selection should survive relay discovery.');
  assert(first.runtime.starts.length === 1, 'Overlapping detection events must never start a second runtime.');
  assert(first.runtime.healthRequests >= requestsBeforeDetection + 2, 'Startup must include successful discovery and a final pre-start health check.');
  const started = first.runtime.starts[0];
  assert(new URL(started.gateway).origin === 'wss://relay.notwyd.app', 'Game traffic must use the encrypted remote relay.');
  assert(new URL(started.gateway).searchParams.get('token') === 'a'.repeat(64), 'WebSocket endpoint must carry the discovered token.');
  assert(new URL(started.httpProxy).searchParams.get('token') === 'a'.repeat(64), 'HTTP endpoint must carry the discovered token.');
  assert(new URL(started.httpProxy).origin === 'https://relay.notwyd.app', 'HTTP traffic must use the remote relay.');
});

await test('Remote discovery starts while the GPU is loading and folder selection remains available during connection', async ({ cache, frame }) => {
  const loading = await frame({ holdGpu: '1', holdHealth: '1' }, async fixture => {
    await until(() => fixture.runtime.releaseGpu && fixture.runtime.releaseHealth, 'independent GPU and remote requests');
    assert(fixture.runtime.healthRequests === 1 && fixture.runtime.firstHealthInitialized === 0, 'The remote request must begin before WASM initialization.');
    fixture.runtime.releaseGpu();
  });
  assert(!loading.query('input[type=file]').disabled, 'A pending remote connection must not disable choosing files.');
  loading.choose('Selected During Connection', 69);
  const saved = await until(() => cache.getInfo(), 'folder saved while health response is pending');
  assert(saved.root === 'Selected During Connection', 'Selected files must be saved independently of the remote connection.');
  assert(loading.runtime.starts.length === 0, 'The game must wait for the remote connection.');
  loading.runtime.releaseHealth();
  await loading.launch();
  await until(() => loading.runtime.completed === 1, 'manual launch after the remote response arrives');
  assert(loading.runtime.permissionQueries === 0 && loading.runtime.loopbackRequests === 0, 'Remote startup must not access local network permissions or endpoints.');
});

await test('Saved files wait through a missing relay and launch explicitly after recovery', async ({ cache, frame }) => {
  const saved = await cache.save(plan('Saved While Offline', 62));
  const waiting = await frame({ relayMissing: '1' });
  await until(() => /Não foi possível conectar/.test(waiting.query('.relay-status').textContent), 'missing relay explanation');
  assert(waiting.runtime.healthRequests === 1, 'Initial relay discovery should issue one health request.');
  assert(waiting.runtime.created === 0, 'Cached files should not be copied into WASM before the relay is ready.');
  assert(!visible(waiting.query('.folder-selection')), 'Existing files must not need selecting again for a relay failure.');
  assert((await cache.getInfo()).id === saved.id, 'Relay failure must preserve saved files.');
  waiting.runtime.relayAvailable = true;
  await waiting.launch();
  await until(() => waiting.runtime.completed === 1, 'cached launch after periodic relay discovery', 7500);
  assert(waiting.runtime.starts[0].root === saved.root, 'The retry should reuse cached files.');
  assert(waiting.runtime.starts.length === 1, 'Automatic discovery should launch the saved game only once.');
  assert((await cache.getInfo()).id === saved.id, 'Cached launch must leave the cache unchanged.');
});

await test('A folder selected without the relay is shared with an already-open idle launcher', async ({ cache, frame }) => {
  const waiting = await frame({ relayMissing: '1' });
  const selecting = await frame({ relayMissing: '1' });
  selecting.choose('Shared Before Play', 66);
  assert(selecting.fixture.beforeUnloadBlocked(), 'Navigating away during the save must request confirmation.');
  const saved = await until(() => cache.getInfo(), 'offline folder publication');
  await until(() => !selecting.fixture.beforeUnloadBlocked(), 'navigation unblocked after save');
  await until(() => waiting.query('.file-summary').textContent.includes('Shared Before Play'), 'notification refresh in another open page');
  assert(saved.root === 'Shared Before Play', 'The shared generation must retain its selected root.');
  assert(!visible(waiting.query('.folder-selection')), 'An idle page should reuse the publication without a second folder picker.');
  assert(selecting.runtime.starts.length === 0 && waiting.runtime.starts.length === 0, 'Sharing must not require a running game.');
  waiting.runtime.relayAvailable = true;
  waiting.detectRelay();
  await waiting.launch();
  await until(() => waiting.runtime.completed === 1, 'launch from files published by another page');
  assert(waiting.runtime.starts[0].files.every(file => file.bytes[0] === 66), 'The receiver must read the shared file bytes.');
});

await test('Focusing an idle page discovers shared files even without a notification', async ({ cache, frame }) => {
  const waiting = await frame({ relayMissing: '1' });
  await cache.save(plan('Editor Published', 67));
  waiting.detectRelay();
  await until(() => waiting.query('.file-summary').textContent.includes('Editor Published'), 'focus refresh of shared cache');
  assert(waiting.query('input[type=file]').files.length === 0, 'Focus refresh must not require another folder selection.');
  waiting.runtime.relayAvailable = true;
  waiting.detectRelay();
  await waiting.launch();
  await until(() => waiting.runtime.completed === 1, 'focused shared installation starts');
  await cache.save(plan('A New Installation', 68));
  const channel = new BroadcastChannel(`${cache.db.name}:changes`);
  channel.postMessage({ type: 'changed' });
  channel.close();
  waiting.detectRelay();
  await new Promise(resolve => setTimeout(resolve, 50));
  assert(waiting.runtime.starts.length === 1 && waiting.runtime.starts[0].root === 'Editor Published', 'Notifications must not replace a running session.');
});

await test('GPU failures distinguish HTTPS, the API, the adapter and texture support', async ({ frame }) => {
  for (const [option, pattern] of [
    ['insecure', /contexto seguro/],
    ['noGpu', /WebGPU indisponível neste navegador/],
    ['noAdapter', /Nenhuma placa de vídeo/],
    ['noBC', /compressão de texturas BC/],
  ]) {
    const unsupported = await frame({ [option]: '1' });
    assert(pattern.test(unsupported.query('.status').textContent), `Unexpected diagnostic for ${option}.`);
    assert(unsupported.runtime.initialized === 0, 'Unavailable GPU support must not initialize WASM.');
    unsupported.detectRelay();
    await new Promise(resolve => setTimeout(resolve, 25));
    assert(pattern.test(unsupported.query('.status').textContent), 'Focus must preserve the GPU failure diagnostic.');
    assert(unsupported.runtime.healthRequests > 0, 'Remote connection should be independent of game initialization.');
    assert(unsupported.runtime.permissionQueries === 0 && unsupported.runtime.loopbackRequests === 0, 'GPU diagnostics must not introduce local relay discovery.');
  }
});

await test('An incompatible remote service prevents startup without discarding saved files', async ({ cache, frame }) => {
  const saved = await cache.save(plan('Protected Saved Files', 63));
  const incompatible = await frame({ incompatibleRelay: '1' });
  await until(() => /incompatível/.test(incompatible.query('.relay-status').textContent), 'incompatible relay explanation');
  assert(incompatible.runtime.starts.length === 0 && incompatible.runtime.created === 0, 'An incompatible relay must not start the runtime.');
  assert((await cache.getInfo()).id === saved.id, 'Protocol failures must preserve saved files.');
  assert(incompatible.runtime.permissionQueries === 0 && incompatible.runtime.loopbackRequests === 0, 'An incompatible service must not trigger local fallback.');
});

await test('A relay stopping during asset import frees the owner and retries the same files without another selection', async ({ cache, frame }) => {
  const local = await frame({ failHealthAfter: '1' });
  local.choose('Retained Selection', 64);
  await local.launch();
  await until(() => local.runtime.freed === 1, 'owner freed after relay stopped');
  assert(local.runtime.starts.length === 0, 'A relay failure before start must not consume the runtime.');
  const shared = await until(() => cache.getInfo(), 'selected folder saved despite relay interruption');
  assert(shared.root === 'Retained Selection', 'Relay interruption must not prevent sharing selected assets.');
  local.runtime.failHealthAfter = 0;
  local.detectRelay();
  await local.launch();
  await until(() => local.runtime.completed === 1, 'retry after relay restarts');
  const saved = await until(() => cache.getInfo(), 'retry cache save');
  assert(saved.root === 'Retained Selection', 'Relay retry should use the retained local selection.');
  assert(local.runtime.starts.length === 1, 'Only the successful attempt should consume the runtime.');
});

await test('Disposal cancels pending relay discovery and never starts a late session', async ({ cache, frame }) => {
  const saved = await cache.save(plan('Disposed Relay Check', 65));
  const pending = await frame({ holdHealth: '1' });
  await until(() => pending.runtime.releaseHealth, 'held relay health check');
  pending.fixture.controller.dispose();
  pending.runtime.releaseHealth();
  await new Promise(resolve => setTimeout(resolve, 30));
  assert(pending.runtime.created === 0 && pending.runtime.starts.length === 0, 'A disposed launcher must never create or start a runtime.');
  assert((await cache.getInfo()).id === saved.id, 'Disposal during relay discovery must preserve cached files.');
});

await test('The running player shows the connected relay beside the fullscreen control in Portuguese', async ({ cache, frame }) => {
  await cache.save(plan('Connected Relay', 71));
  const running = await frame();
  await running.launch();
  await until(() => running.runtime.completed === 1, 'manual runtime startup');
  const badge = await until(() => {
    const element = running.query('.relay-connection');
    return element?.dataset.state === 'connected' && visible(element) ? element : null;
  }, 'visible connected relay badge');
  const fullscreen = running.query('.fullscreen');
  assert(badge.textContent.trim() === 'Conectado', 'The connected state should be displayed in Portuguese.');
  assert(fullscreen.textContent.trim() === 'Tela cheia', 'The fullscreen button should be displayed in Portuguese.');
  assert(badge.parentElement === fullscreen.parentElement, 'The relay badge and fullscreen button should share the player toolbar.');
  assert(badge.getAttribute('role') === 'status', 'Assistive technology should receive relay status changes.');
  assert(running.runtime.starts.length === 1, 'Showing the badge must not create another game session.');
});

await test('The relay badge detects disconnection and recovery without restarting the game', async ({ cache, frame }) => {
  await cache.save(plan('Interrupted Relay', 72));
  const running = await frame();
  await running.launch();
  await until(() => running.runtime.completed === 1, 'manual runtime startup');
  await until(() => running.query('.relay-connection')?.dataset.state === 'connected', 'initial connected relay');
  running.runtime.relayAvailable = false;
  await until(() => running.query('.relay-connection').dataset.state === 'disconnected', 'automatic disconnected relay badge', 7500);
  assert(running.query('.relay-connection').textContent.trim() === 'Sem conexão', 'A failed health check should display a Portuguese disconnection label.');
  assert(visible(running.query('.nwyd-player')), 'Loss of the remote connection must leave the game visible.');
  assert(!visible(running.query('.restart')), 'A temporary connection failure must not end the game session.');

  running.runtime.relayAvailable = true;
  running.fixture.focusRelayCheck();
  await until(() => running.query('.relay-connection').dataset.state === 'connected', 'recovered relay badge');
  assert(running.runtime.starts.length === 1 && running.runtime.completed === 1, 'Monitoring must never consume another one-shot runtime.');
});

await test('Connection monitoring uses only the fixed remote service without browser permissions', async ({ cache, frame }) => {
  await cache.save(plan('Remote Monitoring', 75));
  const running = await frame();
  await running.launch();
  await until(() => running.runtime.completed === 1, 'manual runtime startup');
  await until(() => running.query('.relay-connection')?.dataset.state === 'connected', 'initial permitted relay');
  const previousRequests = running.runtime.healthRequests;
  running.fixture.focusRelayCheck();
  await until(() => running.runtime.healthRequests > previousRequests, 'remote monitor refresh');
  assert(running.runtime.permissionQueries === 0 && running.runtime.loopbackRequests === 0, 'Monitoring must never query permissions or contact localhost.');
  assert(running.runtime.healthUrls.every(url => url === 'https://relay.notwyd.app/health'), 'Every health request must use the fixed remote host.');
  assert(running.runtime.starts.length === 1, 'Monitoring must leave the current runtime alone.');
});

await test('A relay restart requires a page reload and never replaces a running session token', async ({ cache, frame }) => {
  await cache.save(plan('Restarted Relay', 73));
  const running = await frame();
  await running.launch();
  await until(() => running.runtime.completed === 1, 'manual runtime startup');
  await until(() => running.query('.relay-connection')?.dataset.state === 'connected', 'initial relay token');
  running.runtime.relayToken = 'b'.repeat(64);
  running.fixture.focusRelayCheck();
  await until(() => running.query('.relay-connection').dataset.state === 'restarted', 'relay restart warning');
  assert(running.query('.relay-connection').textContent.trim() === 'Conexão reiniciada — recarregue a página', 'A changed relay token should explain that the page needs reloading.');
  assert(new URL(running.runtime.starts[0].gateway).searchParams.get('token') === 'a'.repeat(64), 'The running game must retain the token with which it started.');
  assert(running.runtime.starts.length === 1, 'A relay restart must not start a second runtime.');

  // A later health response cannot repair already broken game sockets.
  running.runtime.relayToken = 'a'.repeat(64);
  running.fixture.focusRelayCheck();
  await new Promise(resolve => setTimeout(resolve, 50));
  assert(running.query('.relay-connection').dataset.state === 'restarted', 'The reload warning must remain until the page is reloaded.');
});

await test('Disposing a running player cancels health checks and stops its relay monitor', async ({ cache, frame }) => {
  await cache.save(plan('Disposed Monitor', 74));
  const running = await frame();
  await running.launch();
  await until(() => running.runtime.completed === 1, 'manual runtime startup');
  await until(() => running.query('.relay-connection')?.dataset.state === 'connected', 'running relay monitor');
  running.runtime.holdNextHealth = true;
  running.fixture.focusRelayCheck();
  await until(() => running.runtime.releaseHealth, 'held monitor health check');
  const requestsBeforeDisposal = running.runtime.healthRequests;
  running.fixture.controller.dispose();
  await until(() => running.runtime.healthAborts === 1, 'monitor health check cancellation');
  running.runtime.releaseHealth();
  running.fixture.focusRelayCheck();
  // Exceed one monitor period to catch an interval left behind after disposal.
  await new Promise(resolve => setTimeout(resolve, 5200));
  assert(running.runtime.healthRequests === requestsBeforeDisposal, 'Disposal must remove focus checks and cancel the recurring monitor.');
  assert(running.runtime.starts.length === 1, 'A late health result must not start another game.');
});

await test('Game closure shows a manual restart and reuses the cached folder after one reload request', async ({ cache, frame }) => {
  const saved = await cache.save(plan('Restartable Game', 81));
  const running = await frame();
  await running.launch();
  await until(() => running.runtime.completed === 1, 'manual runtime startup');
  await until(() => running.runtime.completed === 1, 'game ready before closure');
  running.fixture.closeGame();
  await until(() => running.query('h1').textContent === 'Jogo encerrado', 'closed game screen');
  assert(visible(running.query('.nwyd-launcher')) && !visible(running.query('.nwyd-player')), 'Closing the game must return to the launcher.');
  for (const selector of ['.folder-selection', '.file-summary', '.relay-status', 'progress']) {
    assert(!visible(running.query(selector)), `The ended session must hide ${selector}.`);
  }
  const restart = running.query('.restart');
  assert(visible(restart) && !restart.disabled && restart.textContent.trim() === 'Jogar novamente', 'The ended screen must offer a manual restart.');
  const requests = running.runtime.healthRequests;
  running.detectRelay();
  const changed = new BroadcastChannel(`${cache.db.name}:changes`);
  changed.postMessage({ type: 'changed' });
  changed.close();
  await running.fixture.controller.refreshCache();
  await new Promise(resolve => setTimeout(resolve, 50));
  assert(running.runtime.starts.length === 1 && running.runtime.healthRequests === requests, 'Focus and shared-cache events must not restart a closed session.');
  assert(running.runtime.reloads === 0, 'Closure must wait for the user before reloading.');
  restart.click();
  restart.click();
  assert(running.runtime.reloads === 1 && restart.disabled, 'Repeated restart clicks must request exactly one reload.');
  const next = await frame();
  await until(() => next.runtime.completed === 1, 'new page reuses the saved installation');
  assert(next.runtime.starts[0].root === saved.root && next.runtime.starts[0].files.every(file => file.bytes[0] === 81), 'A fresh runtime must reuse the cached files without choosing the folder again.');
  assert(next.query('input[type=file]').files.length === 0, 'Restart must not require another folder selection.');
});

await test('Closing during startup keeps the ended screen when the pending start resolves', async ({ cache, frame }) => {
  await cache.save(plan('Closing During Start', 82));
  const starting = await frame({ holdStart: '1' });
  await starting.launch();
  await until(() => starting.runtime.releaseStart, 'held runtime startup');
  starting.fixture.closeGame();
  assert(starting.query('h1').textContent === 'Jogo encerrado', 'A closure before startup settles must still end the session.');
  starting.runtime.releaseStart();
  await until(() => starting.runtime.completed === 1, 'late startup resolution');
  await new Promise(resolve => setTimeout(resolve, 20));
  assert(visible(starting.query('.nwyd-launcher')) && !visible(starting.query('.nwyd-player')), 'A late startup result must not reveal the closed player.');
  assert(visible(starting.query('.restart')) && starting.runtime.starts.length === 1, 'The closed session must continue to require manual restart.');
});

await test('An error followed by closure preserves the interruption message and restart action', async ({ cache, frame }) => {
  await cache.save(plan('Interrupted Game', 83));
  const running = await frame();
  await running.launch();
  await until(() => running.runtime.completed === 1, 'manual runtime startup');
  await until(() => running.runtime.completed === 1, 'game ready before interruption');
  running.fixture.failGame('A sessão terminou por uma falha simulada.');
  running.fixture.closeGame();
  running.detectRelay();
  await new Promise(resolve => setTimeout(resolve, 30));
  assert(running.query('h1').textContent === 'Jogo interrompido', 'The close event must preserve the error state.');
  assert(running.query('.status').textContent.includes('A sessão terminou por uma falha simulada.'), 'The original runtime failure must remain visible.');
  assert(visible(running.query('.restart')) && !visible(running.query('.nwyd-player')), 'An interrupted session must offer manual restart.');
});

await test('Restart waits for an in-progress asset save, then becomes available without losing the cache', async ({ cache, frame }) => {
  const running = await frame({ holdSave: '1' });
  running.choose('Save Before Restart', 84);
  await running.launch();
  await until(() => running.runtime.completed === 1 && running.runtime.releaseSave, 'running game with a held asset save');
  running.fixture.closeGame();
  const restart = running.query('.restart');
  assert(visible(restart) && restart.disabled, 'Restart must remain disabled while selected files are being saved.');
  restart.click();
  assert(running.runtime.reloads === 0, 'A pending save must prevent reloading.');
  running.runtime.releaseSave();
  await until(() => !restart.disabled, 'restart enabled after asset save finishes');
  assert((await cache.getInfo()).root === 'Save Before Restart', 'Closure must allow the complete save to finish.');
  assert(running.query('h1').textContent === 'Jogo encerrado' && !visible(running.query('.folder-selection')), 'Finishing the save must not restore startup controls.');
  restart.click();
  assert(running.runtime.reloads === 1, 'Restart must reload after the shared files are safely saved.');
  const next = await frame();
  await until(() => next.runtime.completed === 1, 'restart restores files saved after closure');
  assert(next.runtime.starts[0].root === 'Save Before Restart' && next.runtime.starts[0].files.every(file => file.bytes[0] === 84), 'A save completed after closure must still be associated with the session that requested restart.');
});

await test('Closure aborts a pending health request and stops further monitoring', async ({ cache, frame }) => {
  await cache.save(plan('Closed Monitor', 85));
  const running = await frame();
  await running.launch();
  await until(() => running.runtime.completed === 1, 'manual runtime startup');
  await until(() => running.runtime.completed === 1, 'game ready before held monitor');
  running.runtime.holdNextHealth = true;
  running.detectRelay();
  await until(() => running.runtime.releaseHealth, 'held health request');
  const requests = running.runtime.healthRequests;
  running.fixture.closeGame();
  await until(() => running.runtime.healthAborts === 1, 'closed session aborts discovery');
  running.runtime.releaseHealth();
  running.detectRelay();
  await new Promise(resolve => setTimeout(resolve, 5200));
  assert(running.runtime.healthRequests === requests, 'A closed session must stop interval and focus probes.');
  assert(running.query('h1').textContent === 'Jogo encerrado' && !visible(running.query('.nwyd-player')), 'A late health response must not replace the ended screen.');
  assert(running.runtime.starts.length === 1, 'Late connection work must not start another runtime.');
});

await test('Closure exits fullscreen only when the game owns it', async ({ cache, frame }) => {
  await cache.save(plan('Fullscreen Game', 86));
  for (const owner of ['player', 'other']) {
    const running = await frame({ mockFullscreen: '1' });
    await running.launch();
    await until(() => running.runtime.completed === 1, 'game ready before fullscreen closure');
    running.fixture.setFullscreen(owner);
    running.fixture.closeGame();
    await new Promise(resolve => setTimeout(resolve, 20));
    assert(running.runtime.fullscreenExits === (owner === 'player' ? 1 : 0), 'The launcher must only leave fullscreen owned by its player.');
    assert(visible(running.query('.restart')), 'Fullscreen cleanup must retain the restart action.');
  }
});

window.launcherTestResults.complete = true;
const { passed, failed } = window.launcherTestResults;
summary.textContent = `${passed} passed, ${failed} failed.`;
summary.className = failed ? 'fail' : 'pass';
document.body.dataset.testStatus = failed ? 'failed' : 'passed';
