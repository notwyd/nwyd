export const RELAY_ORIGIN = 'http://127.0.0.1:8787';
export const RELAY_DOWNLOAD_URL = 'https://github.com/notwyd/nwyd/releases/download/relay-latest/nwyd-relay-windows-x86_64.exe';

export class RelayError extends Error {
  constructor(code, message) {
    super(message);
    this.name = 'RelayError';
    this.code = code;
  }
}

// Querying does not request permission. Older browsers may not recognize this
// granular permission; the discovery request then follows the browser's rules.
export async function relayPermission(permissions = globalThis.navigator?.permissions) {
  try {
    const { state } = await permissions.query({ name: 'loopback-network' });
    return ['granted', 'denied'].includes(state) ? state : 'prompt';
  } catch {
    return 'prompt';
  }
}

export const RELAY_PERMISSION_HELP = 'Permita o acesso à rede local para este endereço nas configurações do site no navegador. A conexão será detectada automaticamente.';

/** Discover the local relay without cookies, redirects, or a cached response. */
export async function connectRelay({
  fetch: fetcher = globalThis.fetch,
  permissions = globalThis.navigator?.permissions,
  signal,
  // Leave time to answer a first-use browser permission prompt.
  timeout = 30000,
} = {}) {
  const controller = new AbortController();
  const abort = () => controller.abort();
  signal?.addEventListener('abort', abort, { once: true });
  let timer;
  let timedOut = false;
  try {
    if (signal?.aborted) throw new RelayError('aborted', 'A conexão com o relay foi cancelada.');
    const health = await Promise.race([
      (async () => {
        const response = await fetcher(`${RELAY_ORIGIN}/health`, {
          method: 'GET', mode: 'cors', cache: 'no-store', credentials: 'omit',
          redirect: 'error', targetAddressSpace: 'loopback', signal: controller.signal,
        });
        if (!response.ok) {
          if (response.status === 403) throw new RelayError('origin', 'O relay não permite o acesso deste site. Abra https://notwyd.app/play com a versão mais recente do relay em execução.');
          throw new RelayError('incompatible', 'Não foi possível identificar o relay local. Baixe e abra a versão mais recente do NWYD Relay e tente novamente.');
        }
        const body = await response.text();
        if (body.length > 8192) throw new RelayError('incompatible', 'O programa que está usando a porta do relay não é uma versão compatível do NWYD Relay. Feche-o e abra a versão mais recente do relay.');
        try { return JSON.parse(body); } catch { throw new RelayError('incompatible', 'O programa que está usando a porta do relay não é uma versão compatível do NWYD Relay. Feche-o e abra a versão mais recente do relay.'); }
      })(),
      new Promise((_, reject) => {
        timer = setTimeout(() => {
          timedOut = true;
          controller.abort();
          reject(new RelayError('timeout', 'O relay não respondeu. Mantenha o NWYD Relay aberto e permita o acesso se o navegador solicitar. A página tentará conectar novamente automaticamente.'));
        }, timeout);
      }),
    ]);
    if (signal?.aborted) throw new RelayError('aborted', 'A conexão com o relay foi cancelada.');
    if (health?.app !== 'nwyd-relay' || health.protocol !== 1 ||
      typeof health.version !== 'string' || !health.version.trim() || health.version.length > 128 ||
      typeof health.token !== 'string' || !/^[A-Za-z0-9_-]{32,256}$/.test(health.token)) {
      throw new RelayError('incompatible', 'O relay local é incompatível com este jogo. Baixe e abra a versão mais recente do NWYD Relay e tente novamente.');
    }
    const gateway = new URL('/game', RELAY_ORIGIN);
    gateway.protocol = 'ws:';
    gateway.searchParams.set('token', health.token);
    const httpProxy = new URL('/http', RELAY_ORIGIN);
    httpProxy.searchParams.set('token', health.token);
    return { gateway: gateway.href, httpProxy: httpProxy.href, version: health.version };
  } catch (error) {
    if (signal?.aborted) throw new RelayError('aborted', 'A conexão com o relay foi cancelada.');
    if (error instanceof RelayError) throw error;
    if (timedOut) throw new RelayError('timeout', 'O relay não respondeu. Mantenha o NWYD Relay aberto e permita o acesso se o navegador solicitar. A página tentará conectar novamente automaticamente.');
    if (await relayPermission(permissions) === 'denied') throw new RelayError('permission', RELAY_PERMISSION_HELP);
    throw new RelayError('unavailable', 'Não foi possível conectar ao NWYD Relay. Abra o relay neste computador e permita o acesso se o navegador solicitar. A página tentará conectar novamente automaticamente.');
  } finally {
    clearTimeout(timer);
    signal?.removeEventListener('abort', abort);
  }
}
