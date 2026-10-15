import { planFiles, importFiles } from './files.mjs';
import { openAssetCache, DEFAULT_CACHE_NAME } from './cache.mjs';
import { connectRelay, relayPermission, RelayError, RELAY_PERMISSION_HELP, RELAY_DOWNLOAD_URL } from './relay.mjs';

/** Mount once per page. Every tab owns its runtime; only cached files are shared. */
export async function mountGame(root, {
  gateway = '', httpProxy = '',
  moduleUrl,
  cacheName = DEFAULT_CACHE_NAME,
} = {}) {
  root.innerHTML = `
    <section class="nwyd-launcher" aria-label="NWYD no navegador">
      <header class="launcher-header">
        <nav class="launcher-wrap launcher-nav" aria-label="Navegação principal">
          <a class="brand" href="https://notwyd.app/" aria-label="NWYD, início"><svg class="brand-mark" viewBox="0 0 24 30" aria-hidden="true"><path d="M3 25V5l18 20V5M3 12l18 13M3 5l18 13"/></svg>NWYD</a>
          <a class="site-link" href="https://notwyd.app/">Conheça o NWYD <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 17 17 7M7 7h10v10"/></svg></a>
        </nav>
      </header>
      <div class="launcher-wrap launcher-main">
        <div class="launcher-intro">
          <p class="eyebrow">Jogue no navegador</p>
          <h1>Seu mundo.<em>No navegador.</em></h1>
          <p class="hero-description">Seu universo, a uma aba de distância. Use os arquivos do jogo que você já tem e continue sua história.</p>
          <div class="local-note">
            <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="4" width="18" height="13" rx="1"/><path d="M8 21h8m-4-4v4M8 10l3 3 5-5"/></svg>
            <p><strong>Seus arquivos ficam neste dispositivo.</strong><span>Na próxima visita, o jogo usa os arquivos salvos.</span></p>
          </div>
          <p class="browser-note">Para computadores com suporte a WebGPU.</p>
        </div>
        <div class="launcher-panel">
          <div class="panel-heading">
            <p class="eyebrow">Sua próxima aventura</p>
            <h2>Prepare sua sessão.</h2>
          </div>
          <ol class="setup-steps">
            <li class="relay-setup">
              <div class="step-heading"><span class="step-number" aria-hidden="true">01</span><h3>Abra o relay</h3></div>
              <p>Mantenha o NWYD Relay aberto neste computador para se conectar ao servidor do jogo.</p>
              <a class="relay-download button button-secondary" href="${RELAY_DOWNLOAD_URL}">Baixar relay para Windows <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 3v12m-5-5 5 5 5-5M4 16v5h16v-5"/></svg></a>
              <p class="relay-help hint">A conexão é automática. Permita o acesso à rede local se o navegador solicitar.</p>
              <p class="relay-status hint" role="status">Verificando o acesso do navegador…</p>
            </li>
            <li class="asset-setup">
              <div class="step-heading"><span class="step-number" aria-hidden="true">02</span><h3>Escolha seus arquivos</h3></div>
              <p class="file-summary hint" hidden></p>
              <div class="folder-selection" hidden>
                <p>Selecione a pasta em que o jogo está instalado. O navegador salva os arquivos para a próxima vez.</p>
                <label class="choose button"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 7V5h6l2 2h10v13H3V7Z"/></svg><span>Escolher pasta do jogo</span><input type="file" webkitdirectory multiple aria-label="Escolher pasta do jogo"></label>
                <p class="file-hint hint">A pasta deve conter <strong>ItemList.bin</strong>, <strong>UI</strong> e <strong>Env</strong>. Limite de 1 GiB.</p>
                <p class="cache-status hint" role="status"></p>
              </div>
              <progress max="1" value="0" hidden aria-label="Carregando arquivos do jogo"></progress>
              <p class="status" role="status" aria-live="polite">Verificando os arquivos salvos do jogo…</p>
            </li>
          </ol>
          <p class="panel-foot"><span aria-hidden="true"></span>O jogo abre assim que tudo estiver pronto.</p>
        </div>
      </div>
      <footer class="launcher-wrap launcher-footer"><p>Reconstruído de verdade. Feito para continuar essa história.</p><span>NWYD · NO NAVEGADOR</span></footer>
    </section>
    <div class="nwyd-player" hidden>
      <canvas tabindex="0" aria-label="Jogo NWYD"></canvas>
      <div class="player-toolbar">
        <p class="relay-connection" data-state="checking" role="status" aria-live="polite">Verificando relay…</p>
        <button class="fullscreen" type="button"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 3H3v5m13-5h5v5M3 16v5h5m13-5v5h-5"/></svg>Tela cheia</button>
      </div>
      <p class="session-status" role="status" hidden></p>
      <div class="notice" role="status" hidden><span></span><button type="button" aria-label="Fechar mensagem">Fechar</button></div>
    </div>`;
  const picker = root.querySelector('input');
  const selection = root.querySelector('.folder-selection');
  const status = root.querySelector('.status');
  const progress = root.querySelector('progress');
  const launcher = root.querySelector('.nwyd-launcher');
  const player = root.querySelector('.nwyd-player');
  const canvas = root.querySelector('canvas');
  const cacheStatus = root.querySelector('.cache-status');
  const fileSummary = root.querySelector('.file-summary');
  const sessionStatus = root.querySelector('.session-status');
  const relayStatus = root.querySelector('.relay-status');
  const relayConnection = root.querySelector('.relay-connection');
  // Hosts may supply an already configured gateway explicitly. Page URL
  // parameters never override the local relay or its authentication token.
  const localRelay = !gateway;
  root.querySelector('.relay-setup').hidden = !localRelay;
  relayConnection.hidden = !localRelay;
  let connection = localRelay ? null : { gateway, httpProxy };
  let wasm;
  let cache;
  let cacheInfo;
  let pendingPlan;
  let initialized = false;
  let showFolder = false;
  let busy = false;
  let checkingRelay = false;
  let started = false;
  let launchBlocked = false;
  let fatalError;
  let disposed = false;
  let relayMonitor;
  let relayRestarted = false;
  const lifetime = new AbortController();
  const listeners = [];
  const on = (target, type, listener) => {
    target.addEventListener(type, listener);
    listeners.push(() => target.removeEventListener(type, listener));
  };
  const needsReload = () => Boolean(fatalError) || Boolean(wasm?.requires_reload());
  const message = error => {
    if (error instanceof DOMException || error instanceof TypeError) {
      console.error(error);
      return {
        QuotaExceededError: 'Não há espaço suficiente no navegador para salvar os arquivos.',
        SecurityError: 'O navegador bloqueou o acesso necessário. Verifique as configurações do site.',
        NotAllowedError: 'O navegador não permitiu esta operação. Verifique as permissões do site.',
        NotReadableError: 'Não foi possível ler os arquivos. Verifique a pasta do jogo e tente novamente.',
        AbortError: 'A operação foi cancelada. Tente novamente.',
        TypeError: 'Não foi possível carregar o jogo. Verifique a conexão e recarregue a página.',
      }[error.name] ?? 'Não foi possível concluir a operação no navegador. Tente novamente.';
    }
    return error?.message ?? (typeof error === 'string' ? error : 'Ocorreu um erro inesperado. Tente novamente.');
  };
  const checkActive = () => {
    if (disposed) throw new Error('A página de inicialização do jogo foi fechada.');
  };
  const setRelayStatus = (state, detail = '') => {
    if (disposed) return;
    const labels = {
      checking: 'Verificando relay…',
      connected: 'Relay conectado',
      disconnected: 'Relay desconectado',
      restarted: 'Relay reiniciado — recarregue a página',
    };
    const label = labels[state];
    relayConnection.dataset.state = state;
    if (relayConnection.textContent !== label) relayConnection.textContent = label;
    relayConnection.title = detail || 'Conexão com o relay local. O status é atualizado automaticamente.';
  };
  const stopRelayMonitor = () => {
    clearInterval(relayMonitor);
    relayMonitor = undefined;
  };
  const fail = error => {
    if (disposed) return;
    status.textContent = message(error);
    status.classList.add('error');
    launcher.hidden = false;
    progress.hidden = true;
  };
  const announce = text => {
    if (disposed) return;
    sessionStatus.textContent = text;
    sessionStatus.hidden = !text;
  };
  const updateControls = () => {
    if (disposed) return;
    picker.disabled = !wasm || busy || checkingRelay || started || needsReload();
    selection.hidden = !initialized || busy || Boolean((cacheInfo || pendingPlan) && !showFolder);
    fileSummary.hidden = !cacheInfo && !pendingPlan;
    fileSummary.textContent = pendingPlan ? `Pasta do jogo selecionada: ${pendingPlan.root}.` : cacheInfo ? `Arquivos salvos do jogo prontos: ${cacheInfo.root}.` : '';
  };
  const idleStatus = () => {
    if (disposed || busy || started || launchBlocked) return;
    status.classList.remove('error');
    status.textContent = !connection && (cacheInfo || pendingPlan)
      ? 'Os arquivos do jogo estão prontos. Aguardando o relay para começar a jogar…'
      : !connection ? 'Abra o relay e escolha a pasta do jogo para começar.'
      : 'Escolha a pasta do jogo para começar a jogar.';
  };
  const begin = text => {
    busy = true;
    status.classList.remove('error');
    status.textContent = text;
    progress.hidden = false;
    progress.value = 0;
    updateControls();
  };
  const reportProgress = label => (read, total) => {
    checkActive();
    progress.value = total ? read / total : 1;
    status.textContent = `${label} ${Math.round(read / 1048576)} / ${Math.round(total / 1048576)} MiB`;
  };
  const checkRelay = async () => {
    const found = await connectRelay({ signal: lifetime.signal });
    checkActive();
    connection = found;
    relayStatus.textContent = `NWYD Relay ${found.version} está pronto. Mantenha o programa aberto enquanto joga.`;
    relayStatus.classList.remove('error');
    setRelayStatus('connected');
    return found;
  };
  const relayFailure = error => {
    if (disposed) return;
    connection = null;
    relayStatus.textContent = message(error);
    relayStatus.classList.add('error');
    setRelayStatus('disconnected', message(error));
  };
  // Own GameFiles until start consumes it. Recheck relay readiness after a long
  // file import so a stopped relay never consumes the one-shot WASM runtime.
  const launch = async game => {
    try {
      checkActive();
      if (localRelay) {
        status.textContent = 'Verificando o relay…';
        await checkRelay();
      }
      checkActive();
      status.textContent = 'Iniciando o jogo…';
      await new Promise(resolve => setTimeout(resolve, 0));
      checkActive();
      player.hidden = false;
      const size = player.getBoundingClientRect();
      canvas.width = Math.max(1, Math.round(size.width * devicePixelRatio));
      canvas.height = Math.max(1, Math.round(size.height * devicePixelRatio));
      const owned = game;
      game = undefined;
      await owned.start(canvas, connection.gateway, connection.httpProxy);
      checkActive();
      if (fatalError) throw new Error(fatalError);
      started = true;
      launcher.hidden = true;
      progress.hidden = true;
      canvas.focus();
    } finally {
      game?.free();
    }
  };
  const launchFailure = error => {
    if (disposed) return;
    player.hidden = true;
    if (error instanceof RelayError) relayFailure(error);
    else {
      showFolder = true;
      launchBlocked = true;
    }
    fail(error);
    if (needsReload()) status.textContent += ' Recarregue a página para tentar novamente.';
  };
  const play = async () => {
    if (disposed || busy || checkingRelay || started || launchBlocked || needsReload() || !wasm || !connection || (!pendingPlan && !cacheInfo)) return;
    const plan = pendingPlan;
    let game;
    begin(plan ? 'Carregando os arquivos locais do jogo…' : 'Carregando os arquivos salvos do jogo…');
    try {
      if (plan) {
        game = new wasm.GameFiles(plan.root);
        await importFiles(plan, game, reportProgress('Carregando arquivos locais…'));
      } else {
        const loaded = await cache.load(rootName => new wasm.GameFiles(rootName), reportProgress('Carregando arquivos salvos…'));
        game = loaded?.game;
        checkActive();
        if (!loaded) {
          cacheInfo = null;
          progress.hidden = true;
          status.textContent = 'Escolha a pasta do jogo para começar a jogar.';
          return;
        }
      }
      checkActive();
      const owned = game;
      game = undefined;
      await launch(owned);
      pendingPlan = undefined;
      // Publish only after successful startup. Existing tabs retain their bytes.
      if (plan && cache) {
        announce('Salvando os arquivos do jogo para a próxima vez…');
        try {
          cacheInfo = await cache.save(plan, (read, total) => {
            checkActive();
            announce(`Salvando arquivos do jogo… ${Math.round(read / 1048576)} / ${Math.round(total / 1048576)} MiB`);
          });
          announce('');
        } catch (error) {
          announce(`Você pode continuar jogando nesta sessão, mas os arquivos não puderam ser salvos: ${message(error)} Escolha a pasta novamente na próxima vez.`);
        }
      } else if (plan) {
        announce('O armazenamento do navegador está indisponível. Escolha a pasta do jogo novamente na próxima vez.');
      }
    } catch (error) {
      game?.free();
      launchFailure(error);
    } finally {
      busy = false;
      updateControls();
    }
  };
  const connect = async () => {
    if (disposed || !initialized || busy || checkingRelay || started || needsReload()) return;
    checkingRelay = true;
    updateControls();
    try {
      if (await relayPermission() === 'denied') throw new RelayError('permission', RELAY_PERMISSION_HELP);
      checkActive();
      await checkRelay();
    } catch (error) {
      relayFailure(error);
    } finally {
      checkingRelay = false;
      updateControls();
    }
    idleStatus();
    if (connection) await play();
  };
  // Once the page is open, discover the relay and keep its status current. A
  // restarted relay has new credentials: do not silently replace a running
  // session's gateway or consume another WASM runtime.
  const refreshRelay = async () => {
    if (!localRelay || !initialized || disposed || checkingRelay || fatalError || (!started && (busy || needsReload())) || relayRestarted) return;
    if (!started) return connect();
    checkingRelay = true;
    try {
      if (await relayPermission() === 'denied') throw new RelayError('permission', RELAY_PERMISSION_HELP);
      checkActive();
      const found = await connectRelay({ signal: lifetime.signal, timeout: 3000 });
      if (disposed || fatalError) return;
      if (found.gateway !== connection.gateway || found.httpProxy !== connection.httpProxy) {
        relayRestarted = true;
        setRelayStatus('restarted', 'O relay foi reiniciado. Recarregue a página para iniciar uma nova sessão.');
        stopRelayMonitor();
      } else setRelayStatus('connected');
    } catch (error) {
      if (!fatalError) setRelayStatus('disconnected', message(error));
    } finally {
      checkingRelay = false;
    }
  };
  on(window, 'focus', () => { void refreshRelay(); });
  on(document, 'visibilitychange', () => {
    if (document.visibilityState === 'visible') void refreshRelay();
  });
  on(window, 'nwyd-error', event => {
    fatalError = event.detail;
    stopRelayMonitor();
    fail(`${event.detail} Recarregue a página para reiniciar.`);
    updateControls();
  });
  on(window, 'nwyd-notice', event => {
    root.querySelector('.notice span').textContent = event.detail;
    root.querySelector('.notice').hidden = false;
  });
  on(root.querySelector('.notice button'), 'click', () => {
    root.querySelector('.notice').hidden = true;
    canvas.focus();
  });
  on(root.querySelector('.fullscreen'), 'click', () => {
    player.requestFullscreen().catch(error => {
      if (!disposed) root.querySelector('.fullscreen').textContent = 'Tela cheia indisponível';
      console.warn(error);
    });
  });
  on(picker, 'change', () => {
    if (!picker.files.length || disposed || busy || checkingRelay || started || !wasm || needsReload()) return;
    try {
      pendingPlan = undefined;
      pendingPlan = planFiles([...picker.files]);
      launchBlocked = false;
      showFolder = false;
      idleStatus();
    } catch (error) {
      showFolder = true;
      fail(error);
      return;
    } finally {
      picker.value = '';
      updateControls();
    }
    void play();
  });
  updateControls();
  await Promise.all([
    (async () => {
      try {
        if (!isSecureContext || !navigator.gpu) throw new Error('WebGPU indisponível. Abra esta página por HTTPS em um navegador para computador com WebGPU ativado.');
        const adapter = await navigator.gpu.requestAdapter({ powerPreference: 'high-performance' });
        checkActive();
        if (!adapter) throw new Error('Nenhuma placa de vídeo com suporte a WebGPU está disponível neste navegador. Tente usar um navegador para computador com aceleração de hardware ativada.');
        if (!adapter.features.has('texture-compression-bc')) throw new Error('Este jogo exige uma placa de vídeo com suporte a WebGPU e compressão de texturas BC. Tente usar um computador com aceleração de hardware ativada.');
        const module = moduleUrl
          ? await import(/* @vite-ignore */ moduleUrl)
          : await import('./pkg/nwyd.js');
        checkActive();
        await module.default();
        checkActive();
        wasm = module;
      } catch (error) { fail(error); }
    })(),
    (async () => {
      try {
        const opened = await openAssetCache({ name: cacheName });
        if (disposed) opened.close();
        else {
          cache = opened;
          cacheInfo = await cache.getInfo();
        }
      } catch (error) {
        if (!disposed) cacheStatus.textContent = `Não é possível salvar arquivos neste navegador: ${message(error)}`;
      }
    })(),
  ]);
  initialized = true;
  updateControls();
  if (!disposed && wasm) {
    idleStatus();
    if (!localRelay) void play();
    else {
      relayStatus.textContent = 'Detectando o relay neste computador…';
      relayMonitor = setInterval(() => { void refreshRelay(); }, 5000);
      void refreshRelay();
    }
  }
  return {
    async refreshCache() {
      if (disposed || busy || checkingRelay || started) return;
      cacheInfo = await cache?.getInfo();
      checkActive();
      updateControls();
      await play();
    },
    dispose() {
      disposed = true;
      stopRelayMonitor();
      lifetime.abort();
      pendingPlan = undefined;
      for (const remove of listeners) remove();
      cache?.close();
    },
  };
}
