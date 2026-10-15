export const MAX_ASSET_BYTES = 1024 * 1024 * 1024;

// Skip unused binaries, screenshots and other files the game never reads. Keep all
// other files, including unknown game-specific extensions and extensionless data.
const EXCLUDED = /\.(?:exe|dll|msi|zip|7z|rar|log|dmp|pdb|mp4|avi|bak|tmp)$/i;

export function planFiles(files) {
  if (!files.length) throw new Error('Selecione a pasta que contém ItemList.bin, UI e Env.');
  const selected = [];
  let root;
  let total = 0;
  const paths = new Set();
  const exactPaths = new Set();
  for (const file of files) {
    const parts = file.webkitRelativePath?.split('/');
    if (!parts || parts.length < 2 || parts.some(p => !p || p === '..' || p === '.' || /[\\\0:]/.test(p))) {
      throw new Error('O navegador não forneceu caminhos relativos válidos. Selecione a pasta do jogo.');
    }
    const folder = parts.shift();
    root ??= folder;
    if (folder !== root) throw new Error('Selecione uma pasta do jogo por vez.');
    const path = parts.join('/');
    // These binaries are parsed as data: version.dll supplies the protocol
    // version, and the game executable supplies cursor artwork from its PE resources.
    const binaryAsset = /^(?:version\.dll|wyd\.exe)$/i.test(path);
    if (parts.some(p => p.startsWith('.')) ||
        /^(?:screenshots?)(?:\/|$)/i.test(path) ||
        (!binaryAsset && EXCLUDED.test(path))) continue;
    if (exactPaths.has(path)) throw new Error(`A pasta selecionada contém um arquivo duplicado: ${path}`);
    exactPaths.add(path);
    if (!Number.isSafeInteger(file.size) || file.size < 0) {
      throw new Error(`O navegador não forneceu um tamanho válido para o arquivo ${path}.`);
    }
    total += file.size;
    if (!Number.isSafeInteger(total) || total > MAX_ASSET_BYTES) {
      throw new Error('Esta pasta contém mais de 1 GiB de arquivos do jogo. A versão para navegador suporta até 1 GiB; selecione uma instalação limpa do jogo ou use a versão para computador.');
    }
    paths.add(path.toLowerCase());
    selected.push({ file, path });
  }
  if (!paths.has('itemlist.bin') || ![...paths].some(p => p.startsWith('ui/')) || ![...paths].some(p => p.startsWith('env/'))) {
    throw new Error('Selecione a pasta principal do jogo, que contém ItemList.bin e as pastas UI e Env.');
  }
  return { root, selected, total };
}

export async function importFiles(plan, game, progress,
  yieldFrame = () => new Promise(resolve => setTimeout(resolve, 0)),
  now = () => performance.now()) {
  let read = 0;
  let lastYield = now();
  for (const { file, path } of plan.selected) {
    const bytes = new Uint8Array(await file.arrayBuffer());
    if (bytes.byteLength !== file.size || read + bytes.byteLength > plan.total) {
      throw new Error(`O arquivo foi alterado durante o carregamento: ${path}. Selecione a pasta novamente.`);
    }
    game.add_file(path, bytes);
    read += bytes.byteLength;
    progress(read, plan.total, path);
    if (now() - lastYield > 16) {
      await yieldFrame();
      lastYield = now();
    }
  }
}
