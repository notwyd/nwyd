import test from 'node:test';
import assert from 'node:assert/strict';
import { connectRelay, relayPermission, RELAY_ORIGIN } from './relay.mjs';

const health = { app: 'nwyd-relay', protocol: 1, version: '0.1.0', token: 'a'.repeat(64) };
const permissions = state => ({ query: async ({ name }) => {
  assert.equal(name, 'loopback-network');
  return { state };
} });
const response = (body = health, status = 200) => ({ ok: status === 200, status, text: async () => JSON.stringify(body) });

test('permission probing uses the granular name and safely gates unsupported browsers', async () => {
  assert.equal(await relayPermission(permissions('granted')), 'granted');
  assert.equal(await relayPermission(permissions('denied')), 'denied');
  assert.equal(await relayPermission(permissions('prompt')), 'prompt');
  assert.equal(await relayPermission({ query: async () => { throw new TypeError('Unsupported'); } }), 'prompt');
  assert.equal(await relayPermission(null), 'prompt');
});

test('discovery identifies the relay and authenticates both transports without cookies or redirect following', async () => {
  const result = await connectRelay({ fetch: async (url, options) => {
    assert.equal(url, `${RELAY_ORIGIN}/health`);
    assert.equal(options.cache, 'no-store');
    assert.equal(options.credentials, 'omit');
    assert.equal(options.mode, 'cors');
    assert.equal(options.redirect, 'error');
    assert.equal(options.targetAddressSpace, 'loopback');
    assert.ok(options.signal instanceof AbortSignal);
    return response();
  } });
  const gateway = new URL(result.gateway);
  assert.equal(gateway.origin, 'ws://127.0.0.1:8787');
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
  await assert.rejects(connectRelay({ fetch: async () => response({}, 404) }), { code: 'incompatible' });
  await assert.rejects(connectRelay({ fetch: async () => response({}, 403) }), { code: 'origin' });
});

test('denied permission and unavailable relay have actionable, distinct outcomes', async () => {
  const fetch = async () => { throw new TypeError('Failed to fetch'); };
  await assert.rejects(connectRelay({ fetch, permissions: permissions('denied') }), { code: 'permission' });
  await assert.rejects(connectRelay({ fetch, permissions: permissions('prompt') }), { code: 'unavailable' });
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
