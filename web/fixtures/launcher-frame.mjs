import { mountGame } from '../play.js';

// Browser-only fixture: these doubles never ship in the playable bundle.
const parameters = new URLSearchParams(location.search);
window.__launcherTestRuntime = {
  initialized: 0,
  created: 0,
  freed: 0,
  completed: 0,
  starts: [],
  healthRequests: 0,
  healthAborts: 0,
  relayToken: 'a'.repeat(64),
  relayAvailable: !parameters.has('relayMissing'),
  relayPermission: parameters.get('permission') ?? 'granted',
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
  value: { query: async ({ name }) => {
    if (name !== 'loopback-network') throw new Error('Unexpected permission query.');
    const state = window.__launcherTestRuntime.relayPermission;
    if (state === 'unsupported') throw new TypeError('Unsupported test permission.');
    return { state };
  } },
});
const originalFetch = window.fetch.bind(window);
window.fetch = async (url, options) => {
  if (url !== 'http://127.0.0.1:8787/health') return originalFetch(url, options);
  const runtime = window.__launcherTestRuntime;
  runtime.healthRequests++;
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
  value: { requestAdapter: async () => ({ features: new Set(['texture-compression-bc']) }) },
});
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

window.__launcherTest = {
  ready: false,
  error: null,
  controller: null,
  runtime: window.__launcherTestRuntime,
  focusRelayCheck() {
    window.dispatchEvent(new Event('focus'));
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
  });
} catch (error) {
  window.__launcherTest.error = error?.stack ?? String(error);
} finally {
  window.__launcherTest.ready = true;
}
