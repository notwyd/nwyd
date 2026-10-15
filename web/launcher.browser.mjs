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
    async frame(options = {}) {
      const url = new URL('./fixtures/launcher-frame.html', import.meta.url);
      url.searchParams.set('cacheName', name);
      for (const [key, value] of Object.entries(options)) url.searchParams.set(key, value);
      const element = document.createElement('iframe');
      element.title = `Independent launcher ${frames.length + 1}`;
      element.src = url;
      document.querySelector('#frames').append(element);
      frames.push(element);
      await until(() => element.contentWindow?.__launcherTest?.ready, 'launcher initialization');
      const fixture = element.contentWindow.__launcherTest;
      assert(!fixture.error, fixture.error);
      await until(() => !element.contentDocument.querySelector('input[type=file]').disabled ||
        fixture.runtime.starts.length || fixture.runtime.releaseRead || fixture.runtime.releaseHealth, 'initial cache lookup');
      return {
        fixture,
        runtime: fixture.runtime,
        query: selector => element.contentDocument.querySelector(selector),
        queries: selector => Array.from(element.contentDocument.querySelectorAll(selector)),
        choose: (...args) => fixture.chooseFolder(...args),
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

await test('An empty cache asks for a folder, successful startup saves it, and other page loads start automatically', async ({ cache, frame }) => {
  const first = await frame({ holdStart: '1' });
  assert(!first.query('input[type=file]').disabled, 'Folder selection should be enabled.');
  assert(visible(first.query('.folder-selection')), 'A page without saved files should offer folder selection.');
  assert(!first.query('.play-cache, .clear-cache, .new-session, .connect-relay'), 'The launcher should not require session-management or relay-connection controls.');
  first.choose('First Game', 17);
  await until(() => first.runtime.starts.length === 1, 'initial WASM start');
  assert(await cache.getInfo() === null, 'An unconfirmed runtime must not publish a cache.');
  first.runtime.releaseStart();
  const info = await until(() => cache.getInfo(), 'cache save after successful startup');
  assert(info.root === 'First Game' && info.count === 3 && info.total === 3, 'The stored folder summary should match the imported files.');
  assert(first.query('.nwyd-launcher').hidden && !first.query('.nwyd-player').hidden, 'Successful startup should reveal the player.');
  assert(first.query('input[type=file]').files.length === 0, 'The launcher should release selected File references.');

  const second = await frame();
  await until(() => second.runtime.completed === 1, 'automatic cached startup');
  assert(second.runtime.starts[0].root === 'First Game', 'Automatic startup should use the saved root.');
  assert(second.runtime.starts[0].files.every(file => file.bytes[0] === 17), 'Automatic startup should import the saved bytes.');
  assert(second.query('input[type=file]').files.length === 0, 'A returning player must not need a folder selection.');
  assert((await cache.getInfo()).id === info.id, 'Playing cached files should not rewrite the cache.');

  const third = await frame();
  await until(() => third.runtime.completed === 1, 'automatic new-session startup');
  assert(third.runtime.starts[0].files.every(file => file.bytes[0] === 17), 'Multiple sessions should import the saved generation independently.');
});

await test('Failed runtime startup preserves the previous cache and never publishes a first cache', async ({ cache, frame }) => {
  const first = await frame({ failStart: '1' });
  first.choose('Rejected First Game', 21);
  await until(() => /Deliberate test runtime startup failure/.test(first.query('.status').textContent), 'failed first launch');
  assert(await cache.getInfo() === null, 'A failed first startup must leave the cache empty.');
  assert(first.query('.nwyd-player').hidden, 'A failed runtime must return to the launcher.');

  const original = await cache.save(plan('Known Good Game', 22));
  const replacement = await frame({ failStart: '1' });
  await until(() => replacement.runtime.starts.length === 1 && !replacement.query('input[type=file]').disabled, 'failed cached startup returning to folder selection');
  replacement.choose('Rejected Replacement', 23);
  await until(() => replacement.runtime.starts.length === 2 && !replacement.query('input[type=file]').disabled, 'failed replacement launch');
  assert(/Deliberate test runtime startup failure/.test(replacement.query('.status').textContent), 'The runtime error should remain visible.');
  const after = await cache.getInfo();
  assert(after.id === original.id && after.root === 'Known Good Game', 'Failed replacement must preserve the previous complete cache.');
});

await test('Unavailable browser storage still permits direct folder play', async ({ cache, frame }) => {
  const direct = await frame({ storageUnavailable: '1' });
  assert(!direct.query('input[type=file]').disabled, 'Storage failure must not disable folder selection.');
  direct.choose('Uncached Game', 31);
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
  await until(() => /não puderam ser salvos/.test(direct.query('.session-status').textContent), 'cache quota warning');
  assert(direct.runtime.completed === 1 && !direct.query('.nwyd-player').hidden, 'Cache write failure must not stop the running game.');
  assert((await cache.getInfo()).id === original.id, 'Quota failure must leave the older cache intact.');
});

await test('Replacing or removing stored files affects future page loads and leaves running sessions unchanged', async ({ cache, frame }) => {
  await cache.save(plan('Original Game', 41));
  const running = await frame();
  await until(() => running.runtime.completed === 1, 'original cached session');
  await cache.save(plan('Replacement Game', 42));
  const replacing = await frame();
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

await test('Disposing during local or cached file loading frees the owner and prevents a late runtime startup', async ({ cache, frame }) => {
  const local = await frame({ holdRead: '1' });
  local.choose('Disposed Local Game', 51);
  await until(() => local.runtime.releaseRead, 'paused local file read');
  local.fixture.controller.dispose();
  local.runtime.releaseRead();
  await until(() => local.runtime.freed === 1, 'local GameFiles owner release');
  assert(local.runtime.starts.length === 0, 'Disposal during local import must prevent late startup.');
  assert(await cache.getInfo() === null, 'A disposed launcher must not save an unstarted installation.');

  const saved = await cache.save(plan('Disposed Cached Game', 52));
  const cached = await frame({ holdRead: '1' });
  await until(() => cached.runtime.releaseRead, 'paused cached file read');
  cached.fixture.controller.dispose();
  cached.runtime.releaseRead();
  await until(() => cached.runtime.freed === 1, 'cached GameFiles owner release');
  assert(cached.runtime.starts.length === 0, 'Disposal during cached import must prevent late startup.');
  assert((await cache.getInfo()).id === saved.id, 'Disposal should preserve the existing saved installation.');
});

await test('Unknown browser permission probes automatically and selected files survive until the relay is ready', async ({ cache, frame }) => {
  const first = await frame({ permission: 'unsupported', relayMissing: '1' });
  await until(() => /Não foi possível conectar/.test(first.query('.relay-status').textContent), 'automatic discovery with unsupported permission query');
  assert(first.runtime.healthRequests === 1, 'An unavailable permission query must still allow an automatic initial relay probe.');
  assert(!first.query('.connect-relay'), 'Relay detection must not require a connect button.');
  first.choose('Waiting For Relay', 61);
  assert(first.runtime.created === 0 && first.runtime.starts.length === 0, 'Selecting files must wait for relay readiness before creating WASM owners.');
  assert(await cache.getInfo() === null, 'Waiting for the relay must not publish unstarted files.');
  first.runtime.relayAvailable = true;
  first.runtime.holdNextHealth = true;
  const requestsBeforeDetection = first.runtime.healthRequests;
  first.detectRelay();
  first.detectRelay();
  await until(() => first.runtime.releaseHealth, 'held automatic relay discovery');
  assert(first.runtime.healthRequests === requestsBeforeDetection + 1, 'Overlapping detection events must share one pending health request.');
  assert(first.runtime.created === 0, 'Held relay discovery must not create a game runtime.');
  first.runtime.releaseHealth();
  await until(() => first.runtime.completed === 1, 'startup after automatic relay detection');
  const saved = await until(() => cache.getInfo(), 'selected files saved after relay connection');
  assert(saved.root === 'Waiting For Relay', 'The original selection should survive relay discovery.');
  assert(first.runtime.starts.length === 1, 'Overlapping detection events must never start a second runtime.');
  assert(first.runtime.healthRequests >= requestsBeforeDetection + 2, 'Startup must include successful discovery and a final pre-start health check.');
  const started = first.runtime.starts[0];
  assert(new URL(started.gateway).origin === 'ws://127.0.0.1:8787', 'Game traffic must use the local relay.');
  assert(new URL(started.gateway).searchParams.get('token') === 'a'.repeat(64), 'WebSocket endpoint must carry the discovered token.');
  assert(new URL(started.httpProxy).searchParams.get('token') === 'a'.repeat(64), 'HTTP endpoint must carry the discovered token.');
});

await test('Saved files wait through a missing relay and start once when the relay is detected automatically', async ({ cache, frame }) => {
  const saved = await cache.save(plan('Saved While Offline', 62));
  const waiting = await frame({ relayMissing: '1' });
  await until(() => /Não foi possível conectar/.test(waiting.query('.relay-status').textContent), 'missing relay explanation');
  assert(waiting.runtime.healthRequests === 1, 'Initial relay discovery should issue one health request.');
  assert(waiting.runtime.created === 0, 'Cached files should not be copied into WASM before the relay is ready.');
  assert(!visible(waiting.query('.folder-selection')), 'Existing files must not need selecting again for a relay failure.');
  assert((await cache.getInfo()).id === saved.id, 'Relay failure must preserve saved files.');
  waiting.runtime.relayAvailable = true;
  await until(() => waiting.runtime.completed === 1, 'cached launch after periodic relay discovery', 7500);
  assert(waiting.runtime.starts[0].root === saved.root, 'The retry should reuse cached files.');
  assert(waiting.runtime.starts.length === 1, 'Automatic discovery should launch the saved game only once.');
  assert((await cache.getInfo()).id === saved.id, 'Cached launch must leave the cache unchanged.');
});

await test('Denied permissions and incompatible relays prevent startup without discarding saved files', async ({ cache, frame }) => {
  const saved = await cache.save(plan('Protected Saved Files', 63));
  const denied = await frame({ permission: 'denied' });
  assert(denied.runtime.healthRequests === 0, 'Denied permission must not probe the relay automatically.');
  await until(() => /configurações do site/.test(denied.query('.relay-status').textContent), 'permission denial instructions');
  denied.detectRelay();
  await new Promise(resolve => setTimeout(resolve, 30));
  assert(denied.runtime.healthRequests === 0, 'A focus event must not bypass a denied permission.');
  assert(denied.runtime.created === 0, 'Denied permission must not consume game memory or runtime.');
  const incompatible = await frame({ incompatibleRelay: '1' });
  await until(() => /incompatível/.test(incompatible.query('.relay-status').textContent), 'incompatible relay explanation');
  assert(incompatible.runtime.starts.length === 0 && incompatible.runtime.created === 0, 'An incompatible relay must not start the runtime.');
  assert((await cache.getInfo()).id === saved.id, 'Permission or protocol failures must preserve saved files.');
  denied.runtime.relayPermission = 'granted';
  denied.detectRelay();
  await until(() => denied.runtime.completed === 1, 'automatic launch after permission is restored');
  assert(denied.runtime.starts.length === 1, 'Restoring permission should start the saved game without another control.');
});

await test('A relay stopping during asset import frees the owner and retries the same files without another selection', async ({ cache, frame }) => {
  const local = await frame({ failHealthAfter: '1' });
  local.choose('Retained Selection', 64);
  await until(() => local.runtime.freed === 1, 'owner freed after relay stopped');
  assert(local.runtime.starts.length === 0, 'A relay failure before start must not consume the runtime.');
  assert(await cache.getInfo() === null, 'A relay failure must not publish the pending selection.');
  local.runtime.failHealthAfter = 0;
  local.detectRelay();
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
  const badge = await until(() => {
    const element = running.query('.relay-connection');
    return element?.dataset.state === 'connected' && visible(element) ? element : null;
  }, 'visible connected relay badge');
  const fullscreen = running.query('.fullscreen');
  assert(badge.textContent.trim() === 'Relay conectado', 'The connected state should be displayed in Portuguese.');
  assert(fullscreen.textContent.trim() === 'Tela cheia', 'The fullscreen button should be displayed in Portuguese.');
  assert(badge.parentElement === fullscreen.parentElement, 'The relay badge and fullscreen button should share the player toolbar.');
  assert(badge.getAttribute('role') === 'status', 'Assistive technology should receive relay status changes.');
  assert(running.runtime.starts.length === 1, 'Showing the badge must not create another game session.');
});

await test('The relay badge detects disconnection and recovery without restarting the game', async ({ cache, frame }) => {
  await cache.save(plan('Interrupted Relay', 72));
  const running = await frame();
  await until(() => running.query('.relay-connection')?.dataset.state === 'connected', 'initial connected relay');
  running.runtime.relayAvailable = false;
  await until(() => running.query('.relay-connection').dataset.state === 'disconnected', 'automatic disconnected relay badge', 7500);
  assert(running.query('.relay-connection').textContent.trim() === 'Relay desconectado', 'A failed health check should display a Portuguese disconnection label.');
  assert(visible(running.query('.nwyd-player')), 'Loss of the local relay must leave the game visible.');

  running.runtime.relayAvailable = true;
  running.fixture.focusRelayCheck();
  await until(() => running.query('.relay-connection').dataset.state === 'connected', 'recovered relay badge');
  assert(running.runtime.starts.length === 1 && running.runtime.completed === 1, 'Monitoring must never consume another one-shot runtime.');
});

await test('Revoking loopback permission updates the running badge without probing the relay', async ({ cache, frame }) => {
  await cache.save(plan('Permission Revoked', 75));
  const running = await frame();
  await until(() => running.query('.relay-connection')?.dataset.state === 'connected', 'initial permitted relay');
  const requestsBeforeRevocation = running.runtime.healthRequests;
  running.runtime.relayPermission = 'denied';
  running.fixture.focusRelayCheck();
  await until(() => running.query('.relay-connection').dataset.state === 'disconnected', 'revoked permission badge');
  assert(running.runtime.healthRequests === requestsBeforeRevocation, 'Revoked permission must not trigger a background request or browser prompt.');
  assert(running.runtime.starts.length === 1, 'Revoking permission must leave the current runtime alone.');
});

await test('A relay restart requires a page reload and never replaces a running session token', async ({ cache, frame }) => {
  await cache.save(plan('Restarted Relay', 73));
  const running = await frame();
  await until(() => running.query('.relay-connection')?.dataset.state === 'connected', 'initial relay token');
  running.runtime.relayToken = 'b'.repeat(64);
  running.fixture.focusRelayCheck();
  await until(() => running.query('.relay-connection').dataset.state === 'restarted', 'relay restart warning');
  assert(running.query('.relay-connection').textContent.trim() === 'Relay reiniciado — recarregue a página', 'A changed relay token should explain that the page needs reloading.');
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

window.launcherTestResults.complete = true;
const { passed, failed } = window.launcherTestResults;
summary.textContent = `${passed} passed, ${failed} failed.`;
summary.className = failed ? 'fail' : 'pass';
document.body.dataset.testStatus = failed ? 'failed' : 'passed';
