import assert from 'node:assert/strict';
import test from 'node:test';
import { createAnalytics, sendAnalytics, disposeAnalytics } from '../crates/nwyd/src/analytics/browser.mjs';

function browser(implementation) {
  const requests = [];
  const listeners = new Map();
  const timers = new Map();
  let nextTimer = 0;
  const globals = {
    location: { origin: 'https://notwyd.app', href: 'https://notwyd.app/embedded/games/play/' },
    crypto: { randomUUID: () => '01234567-89ab-4cde-8f01-234567890abc' },
    fetch: (url, options) => {
      const request = { url, options };
      requests.push(request);
      if (implementation) return implementation(request);
      return new Promise((resolve, reject) => Object.assign(request, { resolve, reject }));
    },
    addEventListener: (name, callback) => {
      if (!listeners.has(name)) listeners.set(name, new Set());
      listeners.get(name).add(callback);
    },
    removeEventListener: (name, callback) => listeners.get(name)?.delete(callback),
    setTimeout: (callback, delay) => {
      const id = ++nextTimer;
      timers.set(id, { callback, delay });
      return id;
    },
    clearTimeout: id => timers.delete(id),
  };
  return {
    create: endpoint => createAnalytics(endpoint, globals),
    requests,
    listeners,
    timers,
    emit: (name, event = {}) => {
      for (const listener of [...listeners.get(name) ?? []]) listener(event);
    },
    expire: () => {
      for (const timer of [...timers.values()]) timer.callback();
    },
  };
}

function event(delivery, name = 'gameStarted', user = 'SYNTHETIC_ACCOUNT', extra = {}) {
  return {
    eventName: name,
    userID: user,
    sessionID: delivery.sessionID,
    eventTimestamp: '2000-01-02 03:04:05.000',
    eventParams: { platform: 'GLOBAL_SERVER' },
    ...extra,
  };
}

const encode = value => new TextEncoder().encode(JSON.stringify({ eventList: [value] }));
const payload = request => JSON.parse(new TextDecoder().decode(request.options.body)).eventList[0];
const settle = async () => { for (let turn = 0; turn < 5; turn++) await Promise.resolve(); };

test('lifecycle requests preserve their payload and never use relay credentials', () => {
  const env = browser();
  const delivery = env.create('https://notwyd.app/collect/api/nwyd');
  assert.equal(delivery.sessionID, '01234567-89AB-4CDE-8F01-234567890ABC');
  const started = event(delivery);
  const body = encode(started);
  const original = body.slice();
  assert.equal(sendAnalytics(delivery, body), true);
  body.fill(0);
  assert.deepEqual(env.requests[0].options.body, original);
  assert.deepEqual(payload(env.requests[0]), started);
  const ended = event(delivery, 'gameEnded');
  assert.equal(sendAnalytics(delivery, encode(ended)), true);
  assert.deepEqual(payload(env.requests[1]), ended);
  for (const { url, options } of env.requests) {
    assert.equal(url, 'https://notwyd.app/collect/api/nwyd');
    assert.equal(options.method, 'POST');
    assert.deepEqual(options.headers, { 'Content-Type': 'application/json' });
    assert.equal(options.credentials, 'omit');
    assert.equal(options.cache, 'no-store');
    assert.equal(options.mode, 'same-origin');
    assert.equal(options.redirect, 'error');
    assert.equal(options.referrerPolicy, 'no-referrer');
    assert.equal(options.keepalive, true);
    assert.equal(options.signal.aborted, false);
  }
  disposeAnalytics(delivery);
});

test('collector resolution stays at the origin root under nested page paths', () => {
  const env = browser();
  const delivery = env.create('/collect/api/staging_1-test');
  assert.equal(sendAnalytics(delivery, encode(event(delivery))), true);
  assert.equal(env.requests[0].url, 'https://notwyd.app/collect/api/staging_1-test');
  disposeAnalytics(delivery);
  for (const url of [
    'collect/api/nwyd', '//notwyd.app/collect/api/nwyd', 'https://other.invalid/collect/api/nwyd',
    'http://notwyd.app/collect/api/nwyd', 'https://user@notwyd.app/collect/api/nwyd',
    '/play/collect/api/nwyd', '/collect/api/nwyd?token=x', '/collect/api/nwyd#fragment',
    '/collect/api/nwyd/extra', 'file:///collect/api/nwyd',
  ]) assert.throws(() => env.create(url));
});

test('invalid and non-lifecycle envelopes never create requests or active sessions', () => {
  const env = browser();
  const delivery = env.create('/collect/api/nwyd');
  const valid = event(delivery);
  for (const body of [
    encode(event(delivery, 'messageSent')),
    encode(event(delivery, 'clientDevice')),
    encode(event(delivery, 'gameStarted', '')),
    encode({ ...valid, sessionID: 'OTHER_SESSION' }),
    encode({ ...valid, eventParams: { ...valid.eventParams, message: 'must not be sent' } }),
    encode({ ...valid, token: 'must not be sent' }),
    encode({ ...valid, eventTimestamp: 'invalid' }),
    new TextEncoder().encode(JSON.stringify({ eventList: [valid, valid] })),
    new TextEncoder().encode('{'), new Uint8Array([255]), new Uint8Array(), new Uint8Array(2048),
  ]) assert.equal(sendAnalytics(delivery, body), false);
  env.emit('pagehide', { persisted: false });
  assert.equal(env.requests.length, 0);
  disposeAnalytics(delivery);
});

test('endings require the active account and suppress duplicates and stale sessions', () => {
  const env = browser();
  const delivery = env.create('/collect/api/nwyd');
  assert.equal(sendAnalytics(delivery, encode(event(delivery, 'gameEnded'))), false);
  assert.equal(sendAnalytics(delivery, encode(event(delivery))), true);
  assert.equal(sendAnalytics(delivery, encode(event(delivery, 'gameEnded', 'OTHER_ACCOUNT'))), false);
  assert.equal(sendAnalytics(delivery, encode(event(delivery, 'gameEnded', undefined, { sessionID: 'STALE' }))), false);
  assert.equal(sendAnalytics(delivery, encode(event(delivery, 'gameStarted', 'SECOND_ACCOUNT'))), true);
  assert.equal(sendAnalytics(delivery, encode(event(delivery, 'gameEnded'))), false);
  assert.equal(sendAnalytics(delivery, encode(event(delivery, 'gameStarted', 'SECOND_ACCOUNT'))), true);
  assert.equal(sendAnalytics(delivery, encode(event(delivery, 'gameEnded', 'SECOND_ACCOUNT'))), true);
  assert.equal(sendAnalytics(delivery, encode(event(delivery, 'gameEnded', 'SECOND_ACCOUNT'))), false);
  env.emit('pagehide', { persisted: false });
  assert.deepEqual(env.requests.map(request => payload(request).eventName), ['gameStarted', 'gameStarted', 'gameStarted', 'gameEnded']);
  assert.equal(payload(env.requests.at(-1)).userID, 'SECOND_ACCOUNT');
  disposeAnalytics(delivery);
});

test('pagehide ends once and a background visibility change does not end play', () => {
  const env = browser();
  const delivery = env.create('/collect/api/nwyd');
  sendAnalytics(delivery, encode(event(delivery)));
  env.emit('visibilitychange', { visibilityState: 'hidden' });
  assert.equal(env.requests.length, 1);
  assert.equal(env.listeners.has('visibilitychange'), false);
  env.emit('pagehide', { persisted: false });
  env.emit('pagehide', { persisted: false });
  env.emit('pageshow', { persisted: false });
  assert.equal(env.requests.length, 2);
  assert.equal(payload(env.requests[1]).eventName, 'gameEnded');
  assert.equal(payload(env.requests[1]).userID, 'SYNTHETIC_ACCOUNT');
  assert.equal(payload(env.requests[1]).sessionID, delivery.sessionID);
  assert.notEqual(payload(env.requests[1]).eventTimestamp, '2000-01-02 03:04:05.000');
  assert.match(payload(env.requests[1]).eventTimestamp, /^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.000$/);
  assert.equal(sendAnalytics(delivery, encode(event(delivery, 'gameEnded'))), false);
  disposeAnalytics(delivery);
});

test('BFCache resumes only a session that was active when the page was hidden', () => {
  const env = browser();
  const delivery = env.create('/collect/api/nwyd');
  env.emit('pagehide', { persisted: true });
  env.emit('pageshow', { persisted: true });
  assert.equal(env.requests.length, 0);
  sendAnalytics(delivery, encode(event(delivery)));
  env.emit('pagehide', { persisted: true });
  env.emit('pagehide', { persisted: true });
  env.emit('pageshow', { persisted: true });
  env.emit('pageshow', { persisted: true });
  assert.deepEqual(env.requests.map(request => payload(request).eventName), ['gameStarted', 'gameEnded', 'gameStarted']);
  assert.equal(sendAnalytics(delivery, encode(event(delivery, 'gameEnded'))), true);
  env.emit('pagehide', { persisted: true });
  env.emit('pageshow', { persisted: true });
  assert.equal(env.requests.length, 4);
  disposeAnalytics(delivery);
});

test('request limits and timeouts release capacity without retrying ended sessions', async () => {
  const env = browser();
  const delivery = env.create('/collect/api/nwyd');
  const body = new Uint8Array(2047).fill(32);
  body.set(encode(event(delivery)));
  for (let i = 0; i < 16; i++) assert.equal(sendAnalytics(delivery, body), true);
  assert.equal(sendAnalytics(delivery, encode(event(delivery, 'gameEnded'))), false);
  assert.equal(env.requests.length, 16);
  assert.equal(env.requests.reduce((total, request) => total + request.options.body.length, 0), 16 * 2047);
  assert.equal(env.timers.size, 16);
  assert.ok([...env.timers.values()].every(timer => timer.delay === 3000));
  env.requests[0].reject(new Error('synthetic failure'));
  await settle();
  assert.equal(env.timers.size, 15);
  assert.equal(sendAnalytics(delivery, encode(event(delivery, 'gameEnded'))), false);
  assert.equal(sendAnalytics(delivery, encode(event(delivery))), true);
  env.expire();
  assert.equal(env.timers.size, 0);
  assert.ok(env.requests.slice(1).every(request => request.options.signal.aborted));
  assert.equal(sendAnalytics(delivery, encode(event(delivery, 'gameEnded'))), true);
  env.requests.at(-1).resolve({ status: 500 });
  await settle();
  env.emit('pagehide', { persisted: false });
  assert.equal(env.requests.length, 18);
  assert.equal(env.timers.size, 0);
  disposeAnalytics(delivery);
});

test('synchronous fetch failures release capacity and disposal keeps issued requests alive', () => {
  let throws = true;
  const env = browser(() => {
    if (throws) throw new Error('synthetic synchronous failure');
    return new Promise(() => {});
  });
  const delivery = env.create('/collect/api/nwyd');
  assert.equal(sendAnalytics(delivery, encode(event(delivery))), false);
  assert.equal(env.timers.size, 0);
  throws = false;
  assert.equal(sendAnalytics(delivery, encode(event(delivery, 'gameEnded'))), true);
  disposeAnalytics(delivery);
  disposeAnalytics(delivery);
  assert.equal(env.requests.at(-1).options.signal.aborted, false);
  assert.equal(env.listeners.get('pagehide').size, 0);
  assert.equal(env.listeners.get('pageshow').size, 0);
  assert.equal(sendAnalytics(delivery, encode(event(delivery))), false);
  env.emit('pagehide', { persisted: true });
  env.emit('pageshow', { persisted: true });
  assert.equal(env.requests.length, 2);
  env.expire();
  assert.equal(env.timers.size, 0);
});

test('successful deliveries release their response body and request capacity', async () => {
  const env = browser();
  const delivery = env.create('/collect/api/nwyd');
  let cancelled = 0;
  assert.equal(sendAnalytics(delivery, encode(event(delivery))), true);
  env.requests[0].resolve({ status: 200, body: { cancel: () => { cancelled++; } } });
  await settle();
  assert.equal(cancelled, 1);
  assert.equal(env.timers.size, 0);
  assert.equal(env.requests[0].options.signal.aborted, false);
  assert.equal(sendAnalytics(delivery, encode(event(delivery, 'gameEnded'))), true);
  disposeAnalytics(delivery);
});
