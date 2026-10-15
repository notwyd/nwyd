const MAX_BYTES = 2047;
const MAX_PENDING = 16;
const MAX_PENDING_BYTES = MAX_BYTES * MAX_PENDING;
const TIMEOUT = 3000;
const states = new WeakMap();

function timestamp() {
  const now = new Date();
  const pad = value => String(value).padStart(2, '0');
  return `${String(now.getFullYear()).padStart(4, '0')}-${pad(now.getMonth() + 1)}-${pad(now.getDate())} ${pad(now.getHours())}:${pad(now.getMinutes())}:${pad(now.getSeconds())}.000`;
}

function keys(value, expected) {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    && Object.keys(value).length === expected.length
    && expected.every(key => Object.hasOwn(value, key));
}

function eventFrom(body, sessionID) {
  try {
    const envelope = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(body));
    if (!keys(envelope, ['eventList']) || !Array.isArray(envelope.eventList) || envelope.eventList.length !== 1) return;
    const event = envelope.eventList[0];
    if (!keys(event, ['eventName', 'userID', 'sessionID', 'eventTimestamp', 'eventParams'])
      || !['gameStarted', 'gameEnded'].includes(event.eventName)
      || typeof event.userID !== 'string' || !event.userID || event.userID.includes('\0')
      || event.sessionID !== sessionID
      || typeof event.eventTimestamp !== 'string'
      || !/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.000$/.test(event.eventTimestamp)
      || !keys(event.eventParams, ['platform']) || event.eventParams.platform !== 'GLOBAL_SERVER') return;
    return event;
  } catch {}
}

function post(state, body) {
  if (state.pending.size >= MAX_PENDING || state.pendingBytes + body.length > MAX_PENDING_BYTES) return false;
  const controller = new AbortController();
  const request = { bytes: body.length, timer: undefined };
  state.pending.add(request);
  state.pendingBytes += request.bytes;
  const finished = () => {
    if (!state.pending.delete(request)) return;
    state.pendingBytes -= request.bytes;
    state.runtime.clearTimeout(request.timer);
  };
  request.timer = state.runtime.setTimeout(() => {
    controller.abort();
    finished();
  }, TIMEOUT);
  try {
    Promise.resolve(state.runtime.fetch(state.endpoint, {
      method: 'POST',
      body,
      headers: { 'Content-Type': 'application/json' },
      credentials: 'omit',
      cache: 'no-store',
      mode: 'same-origin',
      redirect: 'error',
      referrerPolicy: 'no-referrer',
      keepalive: true,
      signal: controller.signal,
    })).then(response => response?.body?.cancel()).catch(() => {}).finally(finished);
  } catch {
    finished();
    return false;
  }
  return true;
}

function sendEvent(delivery, event) {
  return sendAnalytics(delivery, new TextEncoder().encode(JSON.stringify({ eventList: [event] })));
}

export function createAnalytics(endpoint, runtime = globalThis) {
  if (typeof endpoint !== 'string' || (!endpoint.startsWith('/') && !/^[a-z][a-z\d+.-]*:/i.test(endpoint)) || endpoint.startsWith('//')) {
    throw new Error('Analytics requires an absolute or origin-root collector URL.');
  }
  const url = new URL(endpoint, runtime.location.origin);
  if (!['http:', 'https:'].includes(url.protocol) || url.origin !== runtime.location.origin
    || url.username || url.password || url.search || url.hash
    || !/^\/collect\/api\/[A-Za-z0-9_-]+$/.test(url.pathname)) {
    throw new Error('Analytics requires a same-origin collector URL.');
  }
  const delivery = Object.freeze({ sessionID: runtime.crypto.randomUUID().toUpperCase() });
  const state = {
    endpoint: url.href,
    runtime,
    active: undefined,
    suspended: undefined,
    disposed: false,
    pending: new Set(),
    pendingBytes: 0,
  };
  state.hide = event => {
    if (state.disposed) return;
    const active = state.active;
    if (active) {
      state.suspended = event.persisted ? active : undefined;
      sendEvent(delivery, { ...active, eventName: 'gameEnded', eventTimestamp: timestamp() });
    } else if (!event.persisted) {
      state.suspended = undefined;
    }
  };
  state.show = event => {
    if (state.disposed || !event.persisted || !state.suspended) return;
    const active = state.suspended;
    state.suspended = undefined;
    sendEvent(delivery, { ...active, eventName: 'gameStarted', eventTimestamp: timestamp() });
  };
  states.set(delivery, state);
  runtime.addEventListener('pagehide', state.hide);
  runtime.addEventListener('pageshow', state.show);
  return delivery;
}

export function sendAnalytics(delivery, body) {
  const state = states.get(delivery);
  if (!state || state.disposed || !(body instanceof Uint8Array) || !body.length || body.length > MAX_BYTES) return false;
  const bytes = body.slice();
  const event = eventFrom(bytes, delivery.sessionID);
  if (!event) return false;
  if (event.eventName === 'gameStarted') {
    state.active = event;
    state.suspended = undefined;
  } else {
    if (!state.active || state.active.userID !== event.userID || state.active.sessionID !== event.sessionID) return false;
    state.active = undefined;
  }
  return post(state, bytes);
}

export function disposeAnalytics(delivery) {
  const state = states.get(delivery);
  if (!state || state.disposed) return;
  state.disposed = true;
  state.active = undefined;
  state.suspended = undefined;
  state.runtime.removeEventListener('pagehide', state.hide);
  state.runtime.removeEventListener('pageshow', state.show);
}
