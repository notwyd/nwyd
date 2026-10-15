import test from 'node:test';
import assert from 'node:assert/strict';
import { importFiles, MAX_ASSET_BYTES, planFiles } from './files.mjs';

const file = (path, size = 1) => ({
  webkitRelativePath: `Game/${path}`,
  size,
  arrayBuffer: async () => new Uint8Array(size).fill(7).buffer,
});
const required = () => [file('ItemList.bin'), file('UI/Panel.WYS'), file('Env/Zone.TRN')];

test('planning strips only the selected root and preserves game file spelling', () => {
  const plan = planFiles([
    file('ITEMLIST.BIN', 3), file('uI/Panel.WYS', 5), file('eNv/Zone.TRN', 7),
    file('Lang/PT/labels.txt', 9), file('mesh/new-format.custom', 11), file('Data', 13),
  ]);
  assert.equal(plan.root, 'Game');
  assert.equal(plan.total, 48);
  assert.deepEqual(plan.selected.map(({ path }) => path), [
    'ITEMLIST.BIN', 'uI/Panel.WYS', 'eNv/Zone.TRN', 'Lang/PT/labels.txt', 'mesh/new-format.custom', 'Data',
  ]);
});

test('planning drops unused binaries and screenshots but keeps protocol and cursor resources', () => {
  const plan = planFiles([
    ...required(), file('VERSION.DLL', 10), file('wYd.ExE', 20), file('Launcher.exe', MAX_ASSET_BYTES),
    file('backup/WYD.exe', MAX_ASSET_BYTES), file('backup/version.dll', MAX_ASSET_BYTES),
    file('Adobe AIR/runtime.dll', MAX_ASSET_BYTES), file('.git/objects/data', MAX_ASSET_BYTES),
    file('UI/.hidden', MAX_ASSET_BYTES), file('Screenshot/001.jpg', MAX_ASSET_BYTES),
    file('Screenshots/002.bmp', MAX_ASSET_BYTES), file('backup.ZIP', MAX_ASSET_BYTES),
  ]);
  assert.equal(plan.total, 33);
  assert.deepEqual(plan.selected.map(({ path }) => path), [
    'ItemList.bin', 'UI/Panel.WYS', 'Env/Zone.TRN', 'VERSION.DLL', 'wYd.ExE',
  ]);
});

test('planning rejects absent roots and missing required asset directories', () => {
  assert.throws(() => planFiles([]), /Selecione a pasta que contém/);
  for (let missing = 0; missing < 3; missing++) {
    assert.throws(() => planFiles(required().filter((_, i) => i !== missing)), /Selecione a pasta principal do jogo/);
  }
  assert.throws(() => planFiles([file('nested/ItemList.bin'), file('UI/a'), file('Env/a')]), /Selecione a pasta principal do jogo/);
});

test('planning rejects traversal, absolute paths and ambiguous browser paths', () => {
  for (const webkitRelativePath of [
    undefined, '', 'ItemList.bin', '/Game/data', 'Game//data', 'Game/../data',
    'Game/./data', 'Game/UI\\data', 'C:/Game/data', 'Game/data\0.bin',
  ]) {
    assert.throws(() => planFiles([...required(), { webkitRelativePath, size: 1 }]), /caminhos relativos válidos/);
  }
  assert.throws(() => planFiles([...required(), { webkitRelativePath: 'Other/UI/a', size: 1 }]), /uma pasta do jogo por vez/);
});

test('duplicate exact paths fail before import while case collisions reach native arbitration', () => {
  assert.throws(() => planFiles([...required(), file('UI/Panel.WYS')]), /arquivo duplicado/);
  const plan = planFiles([...required(), file('ui/panel.wys')]);
  assert.equal(plan.selected.length, 4);
});

test('the memory cap uses selected file sizes and includes the exact boundary', () => {
  assert.equal(planFiles([...required(), file('mesh/large', MAX_ASSET_BYTES - 3)]).total, MAX_ASSET_BYTES);
  assert.throws(() => planFiles([...required(), file('mesh/large', MAX_ASSET_BYTES - 2)]), /mais de 1 GiB/);
  for (const invalidSize of [-1, NaN, Infinity, 1.5, Number.MAX_SAFE_INTEGER + 1]) {
    assert.throws(() => planFiles([...required(), file('mesh/invalid', invalidSize)]), /tamanho válido para o arquivo/);
  }
});

test('import transfers files sequentially, reports exact progress and yields for rendering', async () => {
  const events = [];
  const selectedFiles = required().map((entry) => ({ ...entry, arrayBuffer: async () => {
    events.push(`read:${entry.webkitRelativePath}`);
    return entry.arrayBuffer();
  } }));
  const plan = planFiles(selectedFiles);
  const progress = [];
  let clock = 0;
  let yields = 0;
  await importFiles(plan, { add_file(path, bytes) {
    events.push(`add:${path}`);
    assert.deepEqual([...bytes], [7]);
  } }, (...update) => progress.push(update), async () => { yields++; }, () => (clock += 20));
  assert.deepEqual(events, [
    'read:Game/ItemList.bin', 'add:ItemList.bin',
    'read:Game/UI/Panel.WYS', 'add:UI/Panel.WYS',
    'read:Game/Env/Zone.TRN', 'add:Env/Zone.TRN',
  ]);
  assert.deepEqual(progress, [[1, 3, 'ItemList.bin'], [2, 3, 'UI/Panel.WYS'], [3, 3, 'Env/Zone.TRN']]);
  assert.equal(yields, 3);
});

test('an unreadable file stops importing and preserves the read error', async () => {
  const unavailable = new Error('selected file is no longer available');
  const entries = required();
  entries[1].arrayBuffer = async () => { throw unavailable; };
  const added = [];
  await assert.rejects(importFiles(planFiles(entries), {
    add_file: path => added.push(path),
  }, () => {}), error => error === unavailable);
  assert.deepEqual(added, ['ItemList.bin']);
});

test('changed file sizes and wasm import failures do not report successful progress', async () => {
  const entries = required();
  entries[0].arrayBuffer = async () => new Uint8Array(2).buffer;
  const unexpected = () => assert.fail('a failed file must not be imported or reported');
  await assert.rejects(importFiles(planFiles(entries), { add_file: unexpected }, unexpected), /arquivo foi alterado/);

  const exhausted = new Error('wasm memory limit');
  await assert.rejects(importFiles(planFiles(required()), {
    add_file() { throw exhausted; },
  }, unexpected), error => error === exhausted);
});
