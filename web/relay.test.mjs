import test from 'node:test';
import assert from 'node:assert/strict';
import { connectRelay, RELAY_ORIGIN } from './relay.mjs';

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
