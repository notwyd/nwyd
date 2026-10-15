// Deliberately tiny runtime used only by launcher.browser.test.html. Production
// mountGame still performs every real DOM, file import, and cache operation.
const state = window.__launcherTestRuntime;

export default async function init() {
  state.initialized++;
}

export function requires_reload() {
  // Real WASM reports its one-shot runtime as consumed after startup. Relay
  // monitoring must continue even though another game cannot start here.
  return state.completed > 0;
}

export class GameFiles {
  constructor(root) {
    this.root = root;
    this.files = [];
    state.created++;
  }

  add_file(path, bytes) {
    this.files.push({ path, bytes: Array.from(bytes) });
  }

  free() {
    state.freed++;
  }

  async start(canvas, gateway, httpProxy) {
    state.starts.push({ root: this.root, files: this.files, canvas, gateway, httpProxy });
    if (state.holdStart) {
      await new Promise(resolve => { state.releaseStart = resolve; });
    }
    if (state.failStart) throw new Error('Deliberate test runtime startup failure.');
    state.completed++;
  }
}
