import test from 'node:test';
import assert from 'node:assert/strict';
import { connectRelay, RELAY_ORIGIN, LOCAL_RELAY_ORIGIN, relayPermission } from './relay.mjs';

const health = { app: 'nwyd-relay', protocol: 1, version: '0.1.0', token: 'a'.repeat(64) };
const response = (body = health, status = 200) => ({ ok: status === 200, status, text: async () => JSON.stringify(body) });

test('discovery uses only the fixed remote relay and authenticates both encrypted transports', async () => {
  assert.equal(RELAY_ORIGIN, 'https://relay.notwyd.app');
  const result = await connectRelay({ fetch: async (url, options) => {
    assert.equal(url, `${RELAY_ORIGIN}/health`);
    assert.equal(options.cache, 'no-store');
    assert.equal(options.credentials, 'omit');
    assert.equal(options.mode, 'cors');
    assert.equal(options.redirect, 'error');
    assert.equal('targetAddressSpace' in options, false);
    assert.ok(options.signal instanceof AbortSignal);
    return response();
  } });
  const gateway = new URL(result.gateway);
  assert.equal(gateway.origin, 'wss://relay.notwyd.app');
  assert.equal(gateway.pathname, '/game');
  assert.equal(gateway.searchParams.get('token'), health.token);
  const proxy = new URL(result.httpProxy);
  assert.equal(proxy.origin, RELAY_ORIGIN);
  assert.equal(proxy.pathname, '/http');
  assert.equal(proxy.searchParams.get('token'), health.token);
});

test('wrong services, incompatible protocols, malformed replies and invalid tokens cannot start a session', async () => {
  for (const body of [null, {}, { ...health, app: 'other' }, { ...health, protocol: 2 },
    { ...health, token: '' }, { ...health, token: 't'.repeat(257) },
    { ...health, token: `${'a'.repeat(32)}&host=evil` }, { ...health, version: '' }]) {
    await assert.rejects(connectRelay({ fetch: async () => response(body) }), { code: 'incompatible' });
  }
  await assert.rejects(connectRelay({ fetch: async () => ({ ok: true, text: async () => '<html>Other app</html>' }) }), { code: 'incompatible' });
  await assert.rejects(connectRelay({ fetch: async () => response({}, 404) }), { code: 'unavailable' });
  await assert.rejects(connectRelay({ fetch: async () => response({}, 403) }), { code: 'origin' });
});

test('a network failure does not consult local network permissions', async () => {
  const fetch = async () => { throw new TypeError('Failed to fetch'); };
  await assert.rejects(connectRelay({ fetch, get permissions() { assert.fail('Remote discovery must not inspect browser permissions'); } }), { code: 'unavailable' });
});

test('local discovery uses the fixed loopback relay and authenticates both transports', async () => {
  assert.equal(LOCAL_RELAY_ORIGIN, 'http://127.0.0.1:8787');
  const result = await connectRelay({ mode: 'local', origin: 'https://untrusted.example', fetch: async (url, options) => {
    assert.equal(url, `${LOCAL_RELAY_ORIGIN}/health`);
    assert.equal(options.targetAddressSpace, 'loopback');
    assert.equal(options.cache, 'no-store');
    assert.equal(options.credentials, 'omit');
    assert.equal(options.mode, 'cors');
    assert.equal(options.redirect, 'error');
    return response();
  } });
  const gateway = new URL(result.gateway);
  assert.equal(gateway.origin, 'ws://127.0.0.1:8787');
  assert.equal(gateway.pathname, '/game');
  assert.equal(gateway.searchParams.get('token'), health.token);
  const proxy = new URL(result.httpProxy);
  assert.equal(proxy.origin, LOCAL_RELAY_ORIGIN);
  assert.equal(proxy.pathname, '/http');
  assert.equal(proxy.searchParams.get('token'), health.token);
});

test('unsupported relay modes fail before accessing the network', async () => {
  for (const mode of ['https://untrusted.example', '', null, '__proto__']) {
    await assert.rejects(connectRelay({ mode, fetch: async () => assert.fail('Unexpected request') }), { code: 'mode' });
  }
});

test('local discovery rejects incompatible services without falling back to the remote relay', async () => {
  for (const body of [null, {}, { ...health, app: 'other' }, { ...health, protocol: 2 }, { ...health, token: '' }]) {
    let requests = 0;
    await assert.rejects(connectRelay({ mode: 'local', fetch: async url => {
      requests++;
      assert.equal(url, `${LOCAL_RELAY_ORIGIN}/health`);
      return response(body);
    } }), { code: 'incompatible' });
    assert.equal(requests, 1);
  }
});

test('local network failures distinguish a confirmed permission denial from an unavailable relay', async () => {
  const fetch = async () => { throw new TypeError('Failed to fetch'); };
  for (const state of ['granted', 'prompt', 'denied']) {
    const permissions = { query: async ({ name }) => {
      assert.equal(name, 'loopback-network');
      return { state };
    } };
    await assert.rejects(connectRelay({ mode: 'local', fetch, permissions }), {
      code: state === 'denied' ? 'permission' : 'unavailable',
    });
  }
  const permissions = { query: async () => { throw new TypeError('Unsupported permission'); } };
  await assert.rejects(connectRelay({ mode: 'local', fetch, permissions }), { code: 'unavailable' });
});

test('the older permission name is queried only when the loopback permission is unsupported', async () => {
  const queried = [];
  assert.equal(await relayPermission({ query: async ({ name }) => {
    queried.push(name);
    if (name === 'loopback-network') throw new TypeError('Unsupported permission');
    return { state: 'denied' };
  } }), 'denied');
  assert.deepEqual(queried, ['loopback-network', 'local-network-access']);
  assert.equal(await relayPermission({ query: async ({ name }) => {
    assert.equal(name, 'loopback-network');
    return { state: 'prompt' };
  } }), 'prompt');
});

test('changing relay selection cancels local discovery without leaking the previous credentials', async () => {
  const controller = new AbortController();
  let respond;
  let requestSignal;
  const local = connectRelay({ mode: 'local', signal: controller.signal, fetch: (_, options) => {
    requestSignal = options.signal;
    return new Promise(resolve => { respond = resolve; });
  } });
  controller.abort();
  const remote = await connectRelay({ mode: 'remote', fetch: async () => response({ ...health, token: 'b'.repeat(64) }) });
  respond(response());
  await assert.rejects(local, { code: 'aborted' });
  assert.equal(requestSignal.aborted, true);
  assert.equal(new URL(remote.gateway).origin, 'wss://relay.notwyd.app');
  assert.equal(new URL(remote.gateway).searchParams.get('token'), 'b'.repeat(64));
});

test('timeouts and cancellation also bound a stalled local permission query', async () => {
  const fetch = async () => { throw new TypeError('Failed to fetch'); };
  const permissions = { query: () => new Promise(() => {}) };
  await assert.rejects(connectRelay({ mode: 'local', timeout: 10, fetch, permissions }), { code: 'timeout' });
  const controller = new AbortController();
  const pending = connectRelay({ mode: 'local', signal: controller.signal, fetch, permissions });
  await new Promise(resolve => setTimeout(resolve, 0));
  controller.abort();
  await assert.rejects(pending, { code: 'aborted' });
});

test('a stalled health response times out and aborts its request', async () => {
  let signal;
  await assert.rejects(connectRelay({ timeout: 10, fetch: async (_, options) => {
    signal = options.signal;
    return { ok: true, text: () => new Promise(() => {}) };
  } }), { code: 'timeout' });
  assert.ok(signal.aborted);
});

test('cancelled discovery never returns connection credentials', async () => {
  const controller = new AbortController();
  controller.abort();
  let requests = 0;
  await assert.rejects(connectRelay({ signal: controller.signal, fetch: async () => { requests++; return response(); } }), { code: 'aborted' });
  assert.equal(requests, 0);
  const during = new AbortController();
  await assert.rejects(connectRelay({ signal: during.signal, fetch: async () => { during.abort(); return response(); } }), { code: 'aborted' });
});

test('cancellation stops waiting even if a stalled response ignores its abort signal', async () => {
  const controller = new AbortController();
  let requestSignal;
  const pending = connectRelay({ signal: controller.signal, fetch: async (_, options) => {
    requestSignal = options.signal;
    return { ok: true, text: () => new Promise(() => {}) };
  } });
  controller.abort();
  await assert.rejects(pending, { code: 'aborted' });
  assert.equal(requestSignal.aborted, true);
});
