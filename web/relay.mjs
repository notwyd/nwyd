export const RELAY_ORIGIN = 'https://relay.notwyd.app';

export class RelayError extends Error {
  constructor(code, message) {
    super(message);
    this.name = 'RelayError';
    this.code = code;
  }
}

/** Discover the site's relay without cookies, redirects, or a cached response. */
export async function connectRelay({
  fetch: fetcher = globalThis.fetch,
  signal,
  timeout = 30000,
} = {}) {
  const controller = new AbortController();
  let rejectAbort;
  const cancelled = new Promise((_, reject) => { rejectAbort = reject; });
  const abort = () => {
    controller.abort();
    rejectAbort(new RelayError('aborted', 'Conexão cancelada.'));
  };
  signal?.addEventListener('abort', abort, { once: true });
  let timer;
  let timedOut = false;
  try {
    if (signal?.aborted) throw new RelayError('aborted', 'Conexão cancelada.');
    const health = await Promise.race([
      (async () => {
        const response = await fetcher(`${RELAY_ORIGIN}/health`, {
          method: 'GET', mode: 'cors', cache: 'no-store', credentials: 'omit',
          redirect: 'error', signal: controller.signal,
        });
        if (!response.ok) {
          if (response.status === 403) throw new RelayError('origin', 'A conexão não está disponível para este endereço.');
          throw new RelayError('unavailable', 'Não foi possível conectar. Tentando novamente automaticamente.');
        }
        const body = await response.text();
        if (body.length > 8192) throw new RelayError('incompatible', 'A conexão recebeu uma resposta incompatível.');
        try { return JSON.parse(body); } catch { throw new RelayError('incompatible', 'A conexão recebeu uma resposta incompatível.'); }
      })(),
      new Promise((_, reject) => {
        timer = setTimeout(() => {
          timedOut = true;
          controller.abort();
          reject(new RelayError('timeout', 'A conexão demorou para responder. Tentando novamente automaticamente.'));
        }, timeout);
      }),
      cancelled,
    ]);
    if (signal?.aborted) throw new RelayError('aborted', 'Conexão cancelada.');
    if (health?.app !== 'nwyd-relay' || health.protocol !== 1 ||
      typeof health.version !== 'string' || !health.version.trim() || health.version.length > 128 ||
      typeof health.token !== 'string' || !/^[A-Za-z0-9_-]{32,256}$/.test(health.token)) {
      throw new RelayError('incompatible', 'A conexão recebeu uma resposta incompatível.');
    }
    const gateway = new URL('/game', RELAY_ORIGIN);
    gateway.protocol = 'wss:';
    gateway.searchParams.set('token', health.token);
    const httpProxy = new URL('/http', RELAY_ORIGIN);
    httpProxy.searchParams.set('token', health.token);
    return { gateway: gateway.href, httpProxy: httpProxy.href, version: health.version };
  } catch (error) {
    if (signal?.aborted) throw new RelayError('aborted', 'Conexão cancelada.');
    if (error instanceof RelayError) throw error;
    if (timedOut) throw new RelayError('timeout', 'A conexão demorou para responder. Tentando novamente automaticamente.');
    throw new RelayError('unavailable', 'Não foi possível conectar. Tentando novamente automaticamente.');
  } finally {
    clearTimeout(timer);
    signal?.removeEventListener('abort', abort);
  }
}
