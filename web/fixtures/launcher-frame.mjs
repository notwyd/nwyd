import { mountGame } from '../play.js';
import { AssetCache } from '../cache.mjs';

// Browser-only fixture: these doubles never ship in the playable bundle.
const parameters = new URLSearchParams(location.search);
window.__launcherTestRuntime = {
  initialized: 0,
  created: 0,
  freed: 0,
  completed: 0,
  reloads: 0,
  fullscreenExits: 0,
  starts: [],
  healthRequests: 0,
  healthAborts: 0,
  permissionQueries: 0,
  loopbackRequests: 0,
  healthUrls: [],
  relayToken: 'a'.repeat(64),
  relayAvailable: !parameters.has('relayMissing'),
  failHealthAfter: Number(parameters.get('failHealthAfter') ?? 0),
  holdStart: parameters.has('holdStart'),
  holdRead: parameters.has('holdRead'),
  failStart: parameters.has('failStart'),
};
if (window.__launcherTestRuntime.holdRead) {
  const arrayBuffer = Blob.prototype.arrayBuffer;
  Blob.prototype.arrayBuffer = async function () {
    const bytes = await arrayBuffer.call(this);
    const runtime = window.__launcherTestRuntime;
    if (!runtime.readHeld) {
      runtime.readHeld = true;
      await new Promise(resolve => { runtime.releaseRead = resolve; });
    }
    return bytes;
  };
}
Object.defineProperty(navigator, 'permissions', {
  configurable: true,
  value: { query: async () => {
    window.__launcherTestRuntime.permissionQueries++;
    throw new Error('Remote relay discovery must not query browser permissions.');
  } },
});
const originalFetch = window.fetch.bind(window);
window.fetch = async (url, options) => {
  const runtime = window.__launcherTestRuntime;
  if (/^https?:\/\/(?:localhost|127\.0\.0\.1|\[::1\])(?::|\/)/.test(String(url))) {
    runtime.loopbackRequests++;
    throw new Error('The launcher must never contact a local relay.');
  }
  if (url !== 'https://relay.notwyd.app/health') return originalFetch(url, options);
  if ('targetAddressSpace' in options) throw new Error('Remote relay discovery must not request local network access.');
  runtime.healthRequests++;
  runtime.healthUrls.push(url);
  runtime.firstHealthInitialized ??= runtime.initialized;
  if ((parameters.has('holdHealth') && runtime.healthRequests === 1) || runtime.holdNextHealth) {
    runtime.holdNextHealth = false;
    await new Promise((resolve, reject) => {
      const abort = () => {
        runtime.healthAborts++;
        reject(new DOMException('Cancelled.', 'AbortError'));
      };
      runtime.releaseHealth = () => {
        options.signal.removeEventListener('abort', abort);
        resolve();
      };
      options.signal.addEventListener('abort', abort, { once: true });
    });
  }
  if (!runtime.relayAvailable || (runtime.failHealthAfter && runtime.healthRequests > runtime.failHealthAfter)) {
    throw new TypeError('Deliberate test relay connection failure.');
  }
  return new Response(JSON.stringify({
    app: 'nwyd-relay', protocol: parameters.has('incompatibleRelay') ? 2 : 1,
    version: '0.1.0-test', token: runtime.relayToken,
  }), { headers: { 'Content-Type': 'application/json' } });
};
Object.defineProperty(navigator, 'gpu', {
  configurable: true,
  value: parameters.has('noGpu') ? undefined : {
    requestAdapter: async () => {
      if (parameters.has('holdGpu')) await new Promise(resolve => { window.__launcherTestRuntime.releaseGpu = resolve; });
      return parameters.has('noAdapter') ? null : {
        features: new Set(parameters.has('noBC') ? [] : ['texture-compression-bc']),
      };
    },
  },
});
if (parameters.has('insecure')) Object.defineProperty(window, 'isSecureContext', { configurable: true, value: false });
if (parameters.has('storageUnavailable')) {
  Object.defineProperty(window, 'indexedDB', {
    configurable: true,
    get() { throw new DOMException('Storage disabled by the test fixture.', 'SecurityError'); },
  });
}
if (parameters.has('quotaOnSave')) {
  const put = IDBObjectStore.prototype.put;
  IDBObjectStore.prototype.put = function (...args) {
    if (this.name === 'files') {
      throw new DOMException('Deliberate test storage quota failure.', 'QuotaExceededError');
    }
    return put.apply(this, args);
  };
}
if (parameters.has('holdSave')) {
  const save = AssetCache.prototype.save;
  AssetCache.prototype.save = function (plan, progress, current) {
    return save.call(this, plan, async (...args) => {
      const runtime = window.__launcherTestRuntime;
      if (!runtime.saveHeld) {
        runtime.saveHeld = true;
        await new Promise(resolve => { runtime.releaseSave = resolve; });
      }
      await progress?.(...args);
    }, current);
  };
}
if (parameters.has('mockFullscreen')) {
  Object.defineProperty(document, 'fullscreenElement', {
    configurable: true,
    get: () => window.__launcherTestRuntime.fullscreenElement ?? null,
  });
  document.exitFullscreen = async () => {
    window.__launcherTestRuntime.fullscreenExits++;
    window.__launcherTestRuntime.fullscreenElement = null;
  };
}

window.__launcherTest = {
  ready: false,
  error: null,
  controller: null,
  runtime: window.__launcherTestRuntime,
  focusRelayCheck() {
    window.dispatchEvent(new Event('focus'));
  },
  closeGame() {
    window.dispatchEvent(new Event('nwyd-closed'));
  },
  failGame(message = 'Falha simulada do jogo.') {
    window.dispatchEvent(new CustomEvent('nwyd-error', { detail: message }));
  },
  setFullscreen(owner) {
    window.__launcherTestRuntime.fullscreenElement = owner === 'player'
      ? document.querySelector('.nwyd-player') : owner === 'other' ? document.body : null;
  },
  beforeUnloadBlocked() {
    const event = new Event('beforeunload', { cancelable: true });
    window.dispatchEvent(event);
    return event.defaultPrevented;
  },
  chooseFolder(root = 'Game', marker = 7) {
    const transfer = new DataTransfer();
    for (const path of ['ItemList.bin', 'UI/Panel.WYS', 'Env/Zone.TRN']) {
      const file = new File([new Uint8Array([marker])], path.split('/').at(-1));
      Object.defineProperty(file, 'webkitRelativePath', { value: `${root}/${path}` });
      transfer.items.add(file);
    }
    const input = document.querySelector('input[type=file]');
    input.files = transfer.files;
    input.dispatchEvent(new Event('change', { bubbles: true }));
  },
};

try {
  window.__launcherTest.controller = await mountGame(document.querySelector('#nwyd'), {
    cacheName: parameters.get('cacheName'),
    moduleUrl: new URL('./launcher-test-wasm.mjs', import.meta.url),
    reload: () => { window.__launcherTestRuntime.reloads++; },
  });
} catch (error) {
  window.__launcherTest.error = error?.stack ?? String(error);
} finally {
  window.__launcherTest.ready = true;
}
