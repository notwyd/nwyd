import { MAX_ASSET_BYTES } from './files.mjs';

export const DEFAULT_CACHE_NAME = 'nwyd-assets';
const DATABASE_VERSION = 1;
const FORMAT_VERSION = 1;
const noProgress = () => {};

export class AssetCacheError extends Error {
  constructor(message, code, options) {
    super(message, options);
    this.name = 'AssetCacheError';
    this.code = code;
  }
}

const corrupt = () => new AssetCacheError('Os arquivos salvos do jogo estão incompletos ou corrompidos. Selecione a pasta do jogo novamente.', 'corrupt');
const validPart = part => typeof part === 'string' && part.length > 0 && part !== '.' && part !== '..' && !/[\\\0:/]/.test(part);
const validPath = path => typeof path === 'string' && path.split('/').every(validPart);
const validSize = size => Number.isSafeInteger(size) && size >= 0 && size <= MAX_ASSET_BYTES;

function validateInfo(value) {
  if (!value || value.formatVersion !== FORMAT_VERSION || typeof value.id !== 'string' || !value.id ||
      !validPart(value.root) || !Number.isSafeInteger(value.count) || value.count < 1 ||
      !validSize(value.total) || !Number.isSafeInteger(value.savedAt) || value.savedAt < 0) throw corrupt();
  return { id: value.id, root: value.root, count: value.count, total: value.total, savedAt: value.savedAt };
}

function validatePlan(plan) {
  if (!plan || !validPart(plan.root) || !Array.isArray(plan.selected) || !plan.selected.length || !validSize(plan.total)) throw corrupt();
  let total = 0;
  const paths = new Set();
  for (const { path, file } of plan.selected) {
    if (!validPath(path) || paths.has(path) || !(file instanceof Blob) || !validSize(file.size)) throw corrupt();
    paths.add(path);
    total += file.size;
  }
  if (total !== plan.total || !validSize(total)) throw corrupt();
  validateRequiredPaths(paths);
}

function validateRequiredPaths(paths) {
  const folded = [...paths].map(path => path.toLowerCase());
  if (!folded.includes('itemlist.bin') || !folded.some(path => path.startsWith('ui/')) || !folded.some(path => path.startsWith('env/'))) throw corrupt();
}

// Resolve on transaction completion, not request success: quota failures can
// abort a transaction after its individual put request has succeeded.
function transaction(db, stores, mode, operation) {
  return new Promise((resolve, reject) => {
    let tx;
    let result;
    try {
      tx = db.transaction(stores, mode);
      tx.oncomplete = () => resolve(result);
      tx.onabort = () => reject(tx.error ?? new AssetCacheError('O salvamento dos arquivos do jogo foi interrompido.', 'aborted'));
      tx.onerror = () => {}; // The abort event carries the final transaction error.
      operation(tx, value => { result = value; });
    } catch (error) {
      if (tx) { try { tx.abort(); } catch {} }
      reject(error);
    }
  });
}

function read(db, store, key) {
  return transaction(db, store, 'readonly', (tx, result) => {
    tx.objectStore(store).get(key).onsuccess = event => result(event.target.result);
  });
}

// Keys, rather than Blob values, keep cleanup memory independent of asset size.
function removeGenerations(db, keep) {
  return transaction(db, 'files', 'readwrite', tx => {
    const store = tx.objectStore('files');
    store.openKeyCursor().onsuccess = event => {
      const cursor = event.target.result;
      if (!cursor) return;
      if (cursor.primaryKey[0] !== keep) store.delete(cursor.primaryKey);
      cursor.continue();
    };
  });
}

/** One installation per origin. Every operation coordinates across tabs. */
export class AssetCache {
  constructor(db, locks, lockName) {
    this.db = db;
    this.locks = locks;
    this.lockName = lockName;
    this.closed = false;
    db.onversionchange = () => this.close();
  }

  withLock(mode, action) {
    return this.locks.request(this.lockName, { mode }, async () => {
      if (this.closed) throw new AssetCacheError('A conexão com os arquivos salvos do jogo foi encerrada. Recarregue a página para acessá-los novamente.', 'closed');
      return action();
    });
  }

  async readInfo() {
    const value = await read(this.db, 'meta', 'active');
    return value === undefined ? null : validateInfo(value);
  }

  getInfo() {
    return this.withLock('shared', () => this.readInfo());
  }

  /** Stage File/Blob copies one at a time, then atomically publish the set. */
  save(plan, progress = noProgress) {
    return this.withLock('exclusive', async () => {
      validatePlan(plan);
      // A terminated tab may leave a partial generation; never reuse its data.
      const active = await read(this.db, 'meta', 'active');
      await removeGenerations(this.db, active?.id);
      const info = {
        id: crypto.randomUUID(), root: plan.root, count: plan.selected.length,
        total: plan.total, savedAt: Date.now(),
      };
      let written = 0;
      try {
        for (let order = 0; order < plan.selected.length; order++) {
          const { file, path } = plan.selected[order];
          // Persist a Blob, not a directory permission/handle or source path.
          const blob = file.slice(0, file.size);
          if (blob.size !== file.size) throw corrupt();
          await transaction(this.db, 'files', 'readwrite', tx => {
            tx.objectStore('files').put({ generation: info.id, order, path, size: blob.size, blob });
          });
          written += blob.size;
          await progress(written, info.total, path);
        }
        await transaction(this.db, 'meta', 'readwrite', tx => {
          tx.objectStore('meta').put({ ...info, formatVersion: FORMAT_VERSION }, 'active');
        });
      } catch (error) {
        // Best effort: on a crash, a later save/clear performs the same cleanup.
        try { await removeGenerations(this.db, active?.id); } catch {}
        throw error;
      }
      // Publication already succeeded. A cleanup failure must not report that
      // the saved installation failed; the next save will collect leftovers.
      try { await removeGenerations(this.db, info.id); } catch (error) {
        console.warn('Não foi possível remover os arquivos antigos do jogo.', error);
      }
      return info;
    });
  }

  /** Create the WASM owner from the same generation that supplies its bytes. */
  load(createGame, progress = noProgress, expectedId) {
    return this.withLock('shared', async () => {
      const info = await this.readInfo();
      if (expectedId !== undefined && info?.id !== expectedId) {
        throw new AssetCacheError('A pasta salva do jogo foi alterada em outra aba. Recarregue a página para usar os arquivos atualizados.', 'changed');
      }
      if (!info) return null;
      let game;
      try {
        game = createGame(info.root, info);
        let loaded = 0;
        const paths = new Set();
        for (let order = 0; order < info.count; order++) {
          const entry = await read(this.db, 'files', [info.id, order]);
          if (!entry || entry.generation !== info.id || entry.order !== order || !validPath(entry.path) ||
              paths.has(entry.path) || !validSize(entry.size) || !(entry.blob instanceof Blob) ||
              entry.blob.size !== entry.size || loaded + entry.size > info.total) throw corrupt();
          const bytes = new Uint8Array(await entry.blob.arrayBuffer());
          if (bytes.byteLength !== entry.size) throw corrupt();
          game.add_file(entry.path, bytes);
          paths.add(entry.path);
          loaded += bytes.byteLength;
          await progress(loaded, info.total, entry.path);
        }
        if (loaded !== info.total) throw corrupt();
        validateRequiredPaths(paths);
        return { game, info };
      } catch (error) {
        game?.free();
        throw error;
      }
    });
  }

  clear() {
    return this.withLock('exclusive', () => transaction(this.db, ['meta', 'files'], 'readwrite', tx => {
      tx.objectStore('meta').clear();
      tx.objectStore('files').clear();
    }));
  }

  close() {
    this.closed = true;
    this.db.close();
  }
}

/** Missing/denied storage safely falls back to the launcher's direct importer. */
export async function openAssetCache({
  indexedDB = globalThis.indexedDB,
  locks = globalThis.navigator?.locks,
  name = DEFAULT_CACHE_NAME,
} = {}) {
  if (!indexedDB || !locks?.request) {
    throw new AssetCacheError('Este navegador não pode salvar os arquivos do jogo para outras abas. Você ainda pode selecionar a pasta para esta sessão.', 'unavailable');
  }
  const db = await new Promise((resolve, reject) => {
    const request = indexedDB.open(name, DATABASE_VERSION);
    let blocked = false;
    request.onupgradeneeded = () => {
      const db = request.result;
      db.createObjectStore('meta');
      const files = db.createObjectStore('files', { keyPath: ['generation', 'order'] });
      files.createIndex('path', ['generation', 'path'], { unique: true });
    };
    request.onblocked = () => {
      blocked = true;
      reject(new AssetCacheError('Outra aba está atualizando os arquivos salvos do jogo. Feche essa aba ou recarregue a página para tentar novamente.', 'blocked'));
    };
    request.onerror = () => reject(request.error);
    request.onsuccess = () => {
      if (blocked) request.result.close();
      else resolve(request.result);
    };
  });
  try {
    // A damaged or incompatible schema is reported without deleting user data.
    if (!db.objectStoreNames.contains('meta') || !db.objectStoreNames.contains('files')) throw corrupt();
    const files = db.transaction('files').objectStore('files');
    if (JSON.stringify(files.keyPath) !== JSON.stringify(['generation', 'order']) || !files.indexNames.contains('path')) throw corrupt();
    return new AssetCache(db, locks, `${name}:access`);
  } catch (error) {
    db.close();
    throw error;
  }
}
