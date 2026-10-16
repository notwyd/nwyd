import { planFiles, importFiles } from './files.mjs';
import { openAssetCache, DEFAULT_CACHE_NAME } from './cache.mjs';
import { connectRelay, RelayError } from './relay.mjs';

/** Mount once per page. Every tab owns its runtime; only cached files are shared. */
export async function mountGame(root, {
  moduleUrl,
  cacheName = DEFAULT_CACHE_NAME,
  reload = () => window.location.reload(),
} = {}) {
  let relayMode = 'remote';
  let resumeOnLoad = false;
  try {
    if (sessionStorage.getItem(`${cacheName}:relay-mode`) === 'local') relayMode = 'local';
    resumeOnLoad = sessionStorage.getItem(`${cacheName}:restart`);
    sessionStorage.removeItem(`${cacheName}:restart`);
  } catch { /* Storage restrictions must not prevent starting the game. */ }
  root.innerHTML = `
    <section class="nwyd-launcher" aria-label="Jogar no navegador">
      <div class="launcher-panel">
        <h1 tabindex="-1">Escolha seus arquivos</h1>
        <p class="local-note">Seus arquivos ficam neste dispositivo e serão reutilizados na próxima visita.</p>
        <div class="asset-setup">
          <p class="file-summary" hidden></p>
          <button class="change-folder secondary" type="button" hidden>Escolher outra pasta</button>
          <div class="folder-selection" hidden>
            <label class="choose button"><span>Escolher pasta do jogo</span><input type="file" webkitdirectory multiple aria-label="Escolher pasta do jogo"></label>
            <p class="file-hint">A pasta deve conter <strong>ItemList.bin</strong>, <strong>UI</strong> e <strong>Env</strong>. Limite de 1 GiB.</p>
          </div>
          <p class="cache-status" role="status"></p>
          <div class="relay-options">
            <label for="relay-mode">Conexão com o servidor</label>
            <select id="relay-mode" class="relay-mode">
              <option value="remote">Relay remoto</option>
              <option value="local">Relay local</option>
            </select>
            <p class="remote-relay-help">Use o relay remoto sem instalar nada.</p>
            <div class="local-relay-help" hidden>
              <ol>
                <li><a href="https://github.com/notwyd/nwyd/releases/tag/relay-latest" target="_blank" rel="noopener noreferrer">Baixe o NWYD Relay no GitHub</a> para seu sistema.</li>
                <li>No Windows, abra <code>nwyd-relay-windows-x86_64.exe</code>. No Linux ou macOS, extraia o pacote e execute <code>./nwyd-relay</code>.</li>
                <li>Aguarde “Pronto para jogar” e mantenha o relay aberto. Se o navegador pedir, permita o acesso à rede local.</li>
              </ol>
            </div>
          </div>
          <progress max="1" value="0" hidden aria-label="Carregando arquivos do jogo"></progress>
          <p class="status" role="status" aria-live="polite">Verificando arquivos salvos…</p>
          <button class="launch button" type="button" disabled>Jogar</button>
          <button class="restart button" type="button" hidden>Jogar novamente</button>
          <p class="relay-status" role="status">Conectando…</p>
        </div>
      </div>
    </section>
    <div class="nwyd-player" hidden>
      <canvas tabindex="0" aria-label="Jogo NWYD"></canvas>
      <div class="player-toolbar">
        <p class="relay-connection" data-state="checking" role="status" aria-live="polite">Conectando…</p>
        <button class="fullscreen" type="button"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 3H3v5m13-5h5v5M3 16v5h5m13-5v5h-5"/></svg>Tela cheia</button>
      </div>
      <p class="session-status" role="status" hidden></p>
      <div class="notice" role="status" hidden><span></span><button type="button" aria-label="Fechar mensagem">Fechar</button></div>
    </div>`;
  const picker = root.querySelector('input[type="file"]');
  const changeFolder = root.querySelector('.change-folder');
  const launchButton = root.querySelector('.launch');
  const relayOptions = root.querySelector('.relay-options');
  const relaySelect = root.querySelector('.relay-mode');
  relaySelect.value = relayMode;
  const selection = root.querySelector('.folder-selection');
  const status = root.querySelector('.status');
  const progress = root.querySelector('progress');
  const launcher = root.querySelector('.nwyd-launcher');
  const heading = root.querySelector('h1');
  const restart = root.querySelector('.restart');
  const player = root.querySelector('.nwyd-player');
  const canvas = root.querySelector('canvas');
  const cacheStatus = root.querySelector('.cache-status');
  const fileSummary = root.querySelector('.file-summary');
  const sessionStatus = root.querySelector('.session-status');
  const relayStatus = root.querySelector('.relay-status');
  const relayConnection = root.querySelector('.relay-connection');
  // Both relay addresses are fixed in relay.mjs; page parameters cannot redirect traffic.
  let connection = null;
  let wasm;
  let cache;
  let cacheInfo;
  let pendingPlan;
  let initialized = false;
  let showFolder = false;
  let busy = false;
  let checkingRelay = false;
  let started = false;
  let closed = false;
  let reloadRequested = false;
  let launchBlocked = false;
  let fatalError;
  let disposed = false;
  let relayMonitor;
  let relayRestarted = false;
  let selectionGeneration = 0;
  let selectedCacheId;
  let sessionSelection;
  let sessionCacheId;
  let savesPending = 0;
  let cacheRefresh;
  let cacheChanges;
  try {
    if (typeof BroadcastChannel === 'function') cacheChanges = new BroadcastChannel(`${cacheName}:changes`);
  } catch { /* Focus and visibility refresh still work without notifications. */ }
  let relayAbort = new AbortController();
  let relayGeneration = 0;
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
    if (disposed || closed) throw new Error('A sessão do jogo foi encerrada.');
  };
  const setRelayStatus = (state, detail = '') => {
    if (disposed || closed) return;
    const labels = {
      checking: 'Conectando…',
      connected: 'Conectado',
      disconnected: 'Sem conexão',
      restarted: 'Conexão reiniciada — recarregue a página',
    };
    const label = labels[state];
    relayConnection.dataset.state = state;
    if (relayConnection.textContent !== label) relayConnection.textContent = label;
    relayConnection.title = detail || 'Conexão com o servidor do jogo.';
  };
  const stopRelayMonitor = () => {
    clearInterval(relayMonitor);
    relayMonitor = undefined;
  };
  const fail = error => {
    if (disposed || closed) return;
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
  const cacheNote = text => {
    if (disposed) return;
    cacheStatus.textContent = text;
    announce(text);
  };
  const persistSelection = async (plan, token) => {
    const current = () => !disposed && token === selectionGeneration;
    if (!cache) {
      cacheNote('O armazenamento do navegador está indisponível. Escolha a pasta do jogo novamente na próxima vez.');
      return;
    }
    savesPending++;
    cacheNote('Salvando os arquivos para o jogo e o editor… Aguarde antes de sair desta página.');
    try {
      const saved = await cache.save(plan, (written, total) => {
        if (current()) cacheNote(`Salvando arquivos compartilhados… ${Math.round(written / 1048576)} / ${Math.round(total / 1048576)} MiB. Aguarde antes de sair desta página.`);
      }, current);
      if (!current()) return;
      cacheInfo = saved;
      selectedCacheId = saved.id;
      if (sessionSelection === token) sessionCacheId = saved.id;
      cacheNote('Arquivos salvos neste navegador, disponíveis no jogo e no editor.');
      try { cacheChanges?.postMessage({ type: 'changed' }); } catch {}
    } catch (error) {
      if (current() && error?.code !== 'cancelled') {
        cacheNote(`Os arquivos estão disponíveis nesta sessão, mas não puderam ser salvos: ${message(error)} Escolha a pasta novamente na próxima vez.`);
      }
    } finally {
      savesPending--;
      updateControls();
    }
  };
  const updateControls = () => {
    if (disposed) return;
    picker.disabled = closed || !wasm || busy || started || needsReload();
    selection.hidden = closed || !initialized || busy || Boolean((cacheInfo || pendingPlan) && !showFolder);
    fileSummary.hidden = closed || (!cacheInfo && !pendingPlan);
    fileSummary.textContent = pendingPlan ? `Pasta do jogo selecionada: ${pendingPlan.root}.` : cacheInfo ? `Arquivos salvos do jogo prontos: ${cacheInfo.root}.` : '';
    changeFolder.hidden = closed || !initialized || (!cacheInfo && !pendingPlan) || showFolder;
    changeFolder.disabled = picker.disabled;
    launchButton.hidden = closed;
    launchButton.disabled = picker.disabled || checkingRelay || !connection || (!cacheInfo && !pendingPlan);
    relayOptions.hidden = closed;
    relaySelect.disabled = closed || busy || started || needsReload();
    root.querySelector('.local-relay-help').hidden = relayMode !== 'local';
    root.querySelector('.remote-relay-help').hidden = relayMode !== 'remote';
    restart.hidden = !closed;
    restart.disabled = savesPending > 0 || reloadRequested;
    relayStatus.hidden = closed;
    if (closed && !restart.disabled && document.activeElement === heading) restart.focus();
  };
  const endSession = () => {
    if (disposed) return;
    const wasClosed = closed;
    closed = true;
    launchBlocked = true;
    stopRelayMonitor();
    relayAbort.abort();
    player.hidden = true;
    launcher.hidden = false;
    progress.hidden = true;
    heading.textContent = fatalError ? 'Jogo interrompido' : 'Jogo encerrado';
    status.textContent = fatalError || 'Você pode iniciar o jogo novamente quando quiser.';
    status.classList.toggle('error', Boolean(fatalError));
    updateControls();
    if (!wasClosed) {
      if (document.fullscreenElement && player.contains(document.fullscreenElement)) {
        void document.exitFullscreen().catch(error => console.warn(error));
      }
      (restart.disabled ? heading : restart).focus();
    }
  };
  const idleStatus = () => {
    if (disposed || closed || !wasm || busy || started || launchBlocked) return;
    status.classList.remove('error');
    status.textContent = (cacheInfo || pendingPlan)
      ? showFolder ? 'Escolha outra pasta ou clique em Jogar para usar a pasta atual.'
        : connection ? 'Arquivos prontos. Clique em Jogar para começar.' : 'Arquivos prontos. Aguardando conexão…'
      : 'Escolha a pasta do jogo para começar.';
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
  const checkRelay = async (generation = relayGeneration) => {
    const found = await connectRelay({ mode: relayMode, signal: relayAbort.signal });
    checkActive();
    if (generation !== relayGeneration) throw new RelayError('aborted', 'Conexão cancelada.');
    connection = found;
    relayStatus.textContent = 'Conectado';
    relayStatus.classList.remove('error');
    setRelayStatus('connected');
    return found;
  };
  const relayFailure = (error, generation = relayGeneration) => {
    if (disposed || closed || generation !== relayGeneration) return;
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
      status.textContent = 'Verificando conexão…';
      await checkRelay();
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
    if (disposed || closed) return;
    if (needsReload()) {
      fatalError = message(error);
      endSession();
      return;
    }
    player.hidden = true;
    if (error instanceof RelayError) relayFailure(error);
    else {
      showFolder = true;
      launchBlocked = true;
    }
    fail(error);
  };
  const play = async () => {
    if (disposed || closed || !initialized || busy || checkingRelay || started || launchBlocked || needsReload() || !wasm || !connection || (!pendingPlan && !cacheInfo)) return;
    const plan = pendingPlan;
    sessionSelection = plan ? selectionGeneration : undefined;
    sessionCacheId = plan ? selectedCacheId : undefined;
    let game;
    begin(plan ? 'Carregando os arquivos locais do jogo…' : 'Carregando os arquivos salvos do jogo…');
    try {
      if (plan) {
        game = new wasm.GameFiles(plan.root);
        await importFiles(plan, game, reportProgress('Carregando arquivos locais…'));
      } else {
        const loaded = await cache.load(rootName => new wasm.GameFiles(rootName), reportProgress('Carregando arquivos salvos…'), cacheInfo.id);
        game = loaded?.game;
        sessionCacheId = loaded?.info.id;
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
    } catch (error) {
      game?.free();
      launchFailure(error);
    } finally {
      busy = false;
      updateControls();
    }
  };
  const resumeRequestedGame = async () => {
    if (!resumeOnLoad || !initialized || !wasm || busy || checkingRelay || !connection || (!pendingPlan && !cacheInfo)) return;
    const sameFiles = cacheInfo?.id === resumeOnLoad;
    resumeOnLoad = false;
    if (sameFiles) await play();
  };
  const connect = async () => {
    if (disposed || closed || busy || checkingRelay || started || needsReload()) return;
    checkingRelay = true;
    const generation = relayGeneration;
    updateControls();
    try {
      checkActive();
      await checkRelay(generation);
    } catch (error) {
      relayFailure(error, generation);
    } finally {
      if (generation === relayGeneration) {
        checkingRelay = false;
        updateControls();
      }
    }
    if (generation !== relayGeneration) return;
    idleStatus();
    await resumeRequestedGame();
  };
  // Once the page is open, discover the relay and keep its status current. A
  // restarted relay has new credentials: do not silently replace a running
  // session's gateway or consume another WASM runtime.
  const refreshRelay = async () => {
    if (!initialized || !wasm || disposed || closed || checkingRelay || fatalError || (!started && (busy || needsReload())) || relayRestarted) return;
    if (!started) return connect();
    checkingRelay = true;
    const generation = relayGeneration;
    try {
      checkActive();
      const found = await connectRelay({ mode: relayMode, signal: relayAbort.signal, timeout: 3000 });
      if (disposed || closed || fatalError || generation !== relayGeneration) return;
      if (found.gateway !== connection.gateway || found.httpProxy !== connection.httpProxy) {
        relayRestarted = true;
        setRelayStatus('restarted', 'A conexão foi reiniciada no servidor. Recarregue a página para jogar novamente.');
        stopRelayMonitor();
      } else setRelayStatus('connected');
    } catch (error) {
      if (!fatalError && generation === relayGeneration) setRelayStatus('disconnected', message(error));
    } finally {
      if (generation === relayGeneration) checkingRelay = false;
    }
  };
  const refreshCache = () => {
    if (disposed || closed || !initialized || busy || started || pendingPlan || !cache) return Promise.resolve();
    if (cacheRefresh) return cacheRefresh;
    cacheRefresh = (async () => {
      try {
        const info = await cache.getInfo();
        if (disposed || closed || busy || started || pendingPlan) return;
        if (info?.id !== cacheInfo?.id) {
          cacheInfo = info;
          launchBlocked = false;
        }
        updateControls();
        idleStatus();
        await resumeRequestedGame();
      } catch (error) {
        if (!disposed && !closed) cacheNote(`Não foi possível carregar os arquivos compartilhados: ${message(error)}`);
      } finally { cacheRefresh = undefined; }
    })();
    return cacheRefresh;
  };
  const refreshAvailable = async () => {
    await refreshCache();
    await refreshRelay();
  };
  if (cacheChanges) on(cacheChanges, 'message', event => {
    if (event.data?.type === 'changed') void refreshCache();
  });
  on(window, 'beforeunload', event => {
    if (savesPending) { event.preventDefault(); event.returnValue = ''; }
  });
  on(window, 'focus', () => { void refreshAvailable(); });
  on(document, 'visibilitychange', () => {
    if (document.visibilityState === 'visible') void refreshAvailable();
  });
  on(window, 'nwyd-error', event => {
    fatalError = message(event.detail);
    endSession();
  });
  on(window, 'nwyd-closed', endSession);
  on(restart, 'click', () => {
    if (disposed || !closed || savesPending || reloadRequested) return;
    // Winit and the WASM runtime are one-shot. A new page restores saved files
    // and connects again, but only after the player explicitly asks to restart.
    reloadRequested = true;
    try {
      if (sessionCacheId) sessionStorage.setItem(`${cacheName}:restart`, sessionCacheId);
      else sessionStorage.removeItem(`${cacheName}:restart`);
    } catch {}
    updateControls();
    reload();
  });
  on(window, 'nwyd-notice', event => {
    if (closed) return;
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
  on(changeFolder, 'click', () => {
    if (picker.disabled) return;
    resumeOnLoad = false;
    showFolder = true;
    idleStatus();
    updateControls();
    picker.focus();
  });
  on(launchButton, 'click', () => {
    resumeOnLoad = false;
    launchBlocked = false;
    void play();
  });
  on(relaySelect, 'change', () => {
    if (relaySelect.disabled || !['remote', 'local'].includes(relaySelect.value)) return;
    resumeOnLoad = false;
    relayMode = relaySelect.value;
    try { sessionStorage.setItem(`${cacheName}:relay-mode`, relayMode); } catch {}
    relayGeneration++;
    relayAbort.abort();
    relayAbort = new AbortController();
    checkingRelay = false;
    connection = null;
    relayRestarted = false;
    relayStatus.textContent = 'Conectando…';
    relayStatus.classList.remove('error');
    setRelayStatus('checking');
    idleStatus();
    void connect();
  });
  on(picker, 'change', () => {
    if (!picker.files.length || disposed || closed || busy || started || !wasm || needsReload()) return;
    try {
      const plan = planFiles([...picker.files]);
      resumeOnLoad = false;
      pendingPlan = plan;
      selectedCacheId = undefined;
      void persistSelection(plan, ++selectionGeneration);
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
  });
  updateControls();
  // Discover the chosen relay independently of WASM and file storage.
  // A slow/unavailable relay must not disable folder selection or saving.
  void connect();
  await Promise.all([
    (async () => {
      try {
        if (!isSecureContext) throw new Error('Este jogo precisa de um contexto seguro. Abra a página por HTTPS ou localhost.');
        if (!navigator.gpu) throw new Error('WebGPU indisponível neste navegador. Use um navegador para computador com suporte a WebGPU e aceleração de hardware ativada.');
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
  // A different page may have published while the WASM module was loading.
  // Do not hold page initialization open while a cached game starts.
  void refreshCache();
  updateControls();
  if (!disposed && !closed && wasm) {
    idleStatus();
    relayMonitor = setInterval(() => { void refreshRelay(); }, 5000);
  }
  return {
    refreshCache,
    dispose() {
      disposed = true;
      stopRelayMonitor();
      relayAbort.abort();
      pendingPlan = undefined;
      for (const remove of listeners) remove();
      cacheChanges?.close();
      cache?.close();
    },
  };
}
