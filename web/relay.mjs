export const RELAY_ORIGIN = 'https://relay.notwyd.app';
export const LOCAL_RELAY_ORIGIN = 'http://127.0.0.1:8787';

export class RelayError extends Error {
  constructor(code, message) {
    super(message);
    this.name = 'RelayError';
    this.code = code;
  }
}

// Only a confirmed denial is reported as a permission problem. The older alias
// is queried only when the browser does not recognize the loopback permission.
export async function relayPermission(permissions = globalThis.navigator?.permissions) {
  for (const name of ['loopback-network', 'local-network-access']) {
    try {
      const { state } = await permissions.query({ name });
      return ['granted', 'denied'].includes(state) ? state : 'prompt';
    } catch { /* Unsupported permission; try the older alias. */ }
  }
  return 'prompt';
}

export const RELAY_PERMISSION_HELP = 'Permita o acesso à rede local para este endereço nas configurações do site no navegador e mantenha o NWYD Relay aberto. A conexão será detectada automaticamente.';

/** Discover the selected relay without cookies, redirects, or cached responses. */
export async function connectRelay(options = {}) {
  const { mode = 'remote', fetch: fetcher = globalThis.fetch, signal, timeout = 30000 } = options;
  if (mode !== 'remote' && mode !== 'local') {
    throw new RelayError('mode', 'Escolha o relay remoto ou local.');
  }
  const local = mode === 'local';
  const origin = local ? LOCAL_RELAY_ORIGIN : RELAY_ORIGIN;
  const unavailable = local
    ? 'Não foi possível conectar ao relay local. Abra o NWYD Relay neste computador e permita o acesso se o navegador solicitar. Tentando novamente automaticamente.'
    : 'Não foi possível conectar. Tentando novamente automaticamente.';
  const incompatible = local
    ? 'O relay local é incompatível com este jogo. Baixe e abra a versão mais recente do NWYD Relay.'
    : 'A conexão recebeu uma resposta incompatível.';
  const timeoutMessage = local
    ? 'O relay local demorou para responder. Mantenha o NWYD Relay aberto e permita o acesso se o navegador solicitar. Tentando novamente automaticamente.'
    : 'A conexão demorou para responder. Tentando novamente automaticamente.';
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
        let response;
        try {
          response = await fetcher(`${origin}/health`, {
            method: 'GET', mode: 'cors', cache: 'no-store', credentials: 'omit',
            redirect: 'error', signal: controller.signal,
            ...(local ? { targetAddressSpace: 'loopback' } : {}),
          });
        } catch (error) {
          if (signal?.aborted || controller.signal.aborted) throw error;
          if (local && await relayPermission(options.permissions) === 'denied') {
            throw new RelayError('permission', RELAY_PERMISSION_HELP);
          }
          throw error;
        }
        if (!response.ok) {
          if (response.status === 403) throw new RelayError('origin', local
            ? 'O relay local não permite o acesso deste site. Abra https://notwyd.app/play com a versão mais recente do NWYD Relay em execução.'
            : 'A conexão não está disponível para este endereço.');
          throw new RelayError('unavailable', unavailable);
        }
        const body = await response.text();
        if (body.length > 8192) throw new RelayError('incompatible', incompatible);
        try { return JSON.parse(body); } catch { throw new RelayError('incompatible', incompatible); }
      })(),
      new Promise((_, reject) => {
        timer = setTimeout(() => {
          timedOut = true;
          controller.abort();
          reject(new RelayError('timeout', timeoutMessage));
        }, timeout);
      }),
      cancelled,
    ]);
    if (signal?.aborted) throw new RelayError('aborted', 'Conexão cancelada.');
    if (health?.app !== 'nwyd-relay' || health.protocol !== 1 ||
      typeof health.version !== 'string' || !health.version.trim() || health.version.length > 128 ||
      typeof health.token !== 'string' || !/^[A-Za-z0-9_-]{32,256}$/.test(health.token)) {
      throw new RelayError('incompatible', incompatible);
    }
    const gateway = new URL('/game', origin);
    gateway.protocol = local ? 'ws:' : 'wss:';
    gateway.searchParams.set('token', health.token);
    const httpProxy = new URL('/http', origin);
    httpProxy.searchParams.set('token', health.token);
    return { gateway: gateway.href, httpProxy: httpProxy.href, version: health.version };
  } catch (error) {
    if (signal?.aborted) throw new RelayError('aborted', 'Conexão cancelada.');
    if (error instanceof RelayError) throw error;
    if (timedOut) throw new RelayError('timeout', timeoutMessage);
    throw new RelayError('unavailable', unavailable);
  } finally {
    clearTimeout(timer);
    signal?.removeEventListener('abort', abort);
  }
}
