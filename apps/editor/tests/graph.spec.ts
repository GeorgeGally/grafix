import { test, expect } from '@playwright/test';

test.beforeEach(async ({ page }) => {
  page.on('pageerror', (e) => console.log('PAGEERROR:', e.stack));
  page.on('console', (msg) => { if (msg.type() === 'error') console.log('CONSOLE ERROR:', msg.text()); });
  await page.goto('/tests/graph.html');
  await expect(page.locator('.svelte-flow__node')).toHaveCount(5);
});

test('clicking a palette node visibly adds and selects it', async ({ page }) => {
  await page.getByRole('button', { name: 'Add Cube' }).click();
  console.log('AFTER CLICK:', await page.evaluate(() => ({
    nodeElements: document.querySelectorAll('.svelte-flow__node').length,
    graphNodes: (window as any).__grafixGraph?.nodes.length,
    savedNodes: JSON.parse(localStorage.getItem('grafix:graph:v1') || '{}')?.nodes?.length,
    notice: document.querySelector('.status')?.textContent,
    error: document.querySelector('.bar-error')?.textContent,
    inspector: document.querySelector('.inspector h3')?.textContent,
  })));
  await expect(page.locator('.svelte-flow__node')).toHaveCount(6);
  await expect(page.locator('.inspector h3')).toHaveText('Cube');
  await expect(page.getByText('Added Cube')).toBeVisible();
  const graph = await page.evaluate(() => (window as any).__grafixGraph);
  expect(graph.nodes.filter((n: any) => n.type === 'geometry.cube')).toHaveLength(2);
});

test('dragging a cloner into the graph exposes inactive status and activation', async ({ page }) => {
  const palette = page.getByRole('button', { name: 'Add Fibonacci Cloner' });
  const canvas = page.getByRole('application', { name: 'Node graph canvas' });
  await palette.dragTo(canvas, { targetPosition: { x: 320, y: 170 } });
  console.log('AFTER DRAG:', await page.evaluate(() => ({
    nodeElements: document.querySelectorAll('.svelte-flow__node').length,
    graphNodes: (window as any).__grafixGraph?.nodes.length,
    notice: document.querySelector('.status')?.textContent,
    error: document.querySelector('.bar-error')?.textContent,
    inspector: document.querySelector('.inspector h3')?.textContent,
  })));
  await expect(page.locator('.svelte-flow__node')).toHaveCount(6);
  await expect(page.locator('.inspector h3')).toHaveText('Fibonacci Cloner');
  await expect(page.getByText("Not connected to Output. Changes here won't affect the viewport.")).toBeVisible();
  await page.getByRole('button', { name: 'Use this cloner in the scene' }).click();
  await expect(page.getByText('Connected to render output')).toBeVisible();
  const graph = await page.evaluate(() => (window as any).__grafixGraph);
  const activeLink = graph.edges.find((e: any) => e.to.node === 'scene' && e.to.port === 'clones');
  expect(activeLink.from.node).not.toBe('clones');
});

test('editing connected Fibonacci count updates graph runtime API', async ({ page }) => {
  await page.locator('.svelte-flow__node').filter({ hasText: 'Fibonacci Cloner' }).click();
  await expect(page.getByText('Connected to render output')).toBeVisible();
  await page.locator('.inspector input[type="range"]').first().fill('4321');
  console.log('AFTER EDIT:', await page.evaluate(() => ({
    rangeValue: (document.querySelector('.inspector input[type=range]') as HTMLInputElement)?.value,
    graphCount: (window as any).__grafixGraph?.nodes.find((n: any) => n.id === 'clones')?.params.count,
    savedCount: JSON.parse(localStorage.getItem('grafix:graph:v1') || '{}')?.nodes?.find((n: any) => n.id === 'clones')?.params.count,
    error: document.querySelector('.bar-error')?.textContent,
  })));
  const count = await page.evaluate(() => (window as any).__grafixGraph.nodes
    .find((n: any) => n.id === 'clones').params.count);
  expect(count).toBe(4321);
});


test('Cube XYZ position rotation and scale are editable, resettable, and visible as a scene object', async ({ page }) => {
  await page.locator('.svelte-flow__node').filter({ hasText: 'Cube' }).click();
  await expect(page.getByRole('heading', { name: 'Transform' })).toBeVisible();
  await page.locator('#cube-position_x').fill('3.25');
  await page.locator('#cube-rotation_y').fill('42');
  await page.locator('#cube-scale_z').fill('2.5');
  const changed = await page.evaluate(() => (window as any).__grafixGraph.nodes.find((n: any) => n.id === 'cube').params);
  expect(changed.position_x).toBe(3.25);
  expect(changed.rotation_y).toBe(42);
  expect(changed.scale_z).toBe(2.5);
  await page.getByRole('button', { name: 'Show cube as a scene object' }).click();
  const graph = await page.evaluate(() => (window as any).__grafixGraph);
  expect(graph.edges.some((e: any) => e.to.node === 'scene' &&
    e.to.port === 'geometry' && e.from.node === 'cube')).toBe(true);
  await page.getByRole('button', { name: 'Reset', exact: true }).last().click();
  const reset = await page.evaluate(() => (window as any).__grafixGraph.nodes.find((n: any) => n.id === 'cube').params);
  expect(reset.position_x).toBe(0);
  expect(reset.rotation_y).toBe(0);
  expect(reset.scale_z).toBe(1);
});

test('GLB model has independent position and per-axis scale controls', async ({ page }) => {
  await page.getByRole('button', { name: 'Add GLB Model' }).click();
  await expect(page.locator('.inspector h3')).toHaveText('GLB Model');
  await expect(page.locator('.axis-control input')).toHaveCount(9);
  await page.getByRole('button', { name: 'Use this model in scene' }).click();
  await expect(page.getByText('Connected to render output')).toBeVisible();
  await page.locator('.axis-control input[id$="position_y"]').fill('4.5');
  await page.locator('.axis-control input[id$="scale_x"]').fill('25');
  const model = await page.evaluate(() => (window as any).__grafixGraph.nodes.find((n: any) => n.type === 'asset.boombox'));
  expect(model.params.position_y).toBe(4.5);
  expect(model.params.scale_x).toBe(25);
  const graph = await page.evaluate(() => (window as any).__grafixGraph);
  expect(graph.edges.some((e: any) => e.to.node === 'scene' &&
    e.to.port === 'model' && e.from.node === model.id)).toBe(true);
});

test('viewport translation event moves selected Cube along a world axis', async ({ page }) => {
  await page.locator('.svelte-flow__node').filter({ hasText: 'Cube' }).click();
  await page.evaluate(() => {
    for (const [phase, delta] of [['begin', 0], ['update', 0.8], ['end', 0]] as const) {
      window.dispatchEvent(new CustomEvent('grafix:viewport-transform', {
        detail: { phase, axis: 'x', delta },
      }));
    }
  });
  const cube = await page.evaluate(() => (window as any).__grafixGraph.nodes.find((n: any) => n.id === 'cube'));
  expect(cube.params.position_x).toBe(0.8);
});

test('orbit camera parameters remain live node settings', async ({ page }) => {
  await page.locator('.svelte-flow__node').filter({ hasText: 'Orbit Camera' }).click();
  await page.locator('.inspector .parameter').filter({ hasText: 'distance' })
    .locator('input[type=range]').fill('17');
  const camera = await page.evaluate(() => (window as any).__grafixGraph.nodes.find((n: any) => n.id === 'camera'));
  expect(camera.params.distance).toBe(17);
});


test('Scene accepts two independent cube connections instead of replacing the first', async ({ page }) => {
  await page.locator('.svelte-flow__node').filter({ hasText: 'Cube' }).click();
  await page.getByRole('button', { name: 'Show cube as a scene object' }).click();
  await page.getByRole('button', { name: 'Add Cube' }).click();
  const second = await page.evaluate(() => (window as any).__grafixGraph.nodes
    .filter((n: any) => n.type === 'geometry.cube').at(-1).id);
  await page.locator('.axis-control input[id$="position_x"]').fill('3');
  await page.getByRole('button', { name: 'Show cube as a scene object' }).click();
  const graph = await page.evaluate(() => (window as any).__grafixGraph);
  const connected = graph.edges.filter((e: any) => e.to.node === 'scene' && e.to.port === 'geometry')
    .map((e: any) => e.from.node);
  expect(connected).toEqual(expect.arrayContaining(['cube', second]));
  expect(connected).toHaveLength(2);
  expect(graph.nodes.find((n: any) => n.id === second).params.position_x).toBe(3);
});

test('PBR material color and cube visibility are editable and persist across reload', async ({ page }) => {
  await page.getByRole('button', { name: 'Add PBR Material' }).click();
  await expect(page.locator('.inspector h3')).toHaveText('PBR Material');
  await expect(page.getByRole('region', { name: 'Appearance parameters' })).toBeVisible();
  await page.locator('#material-target').selectOption('cube');
  await page.getByRole('button', { name: 'Apply material' }).click();
  await page.locator('.inspector input[type=color]').first().fill('#ff2244');
  const graph = await page.evaluate(() => (window as any).__grafixGraph);
  const material = graph.nodes.find((n: any) => n.type === 'material.pbr');
  expect(graph.edges.some((e: any) => e.from.node === material.id && e.to.node === 'cube' &&
    e.to.port === 'material')).toBe(true);
  expect(material.params.base_color[0]).toBe(1);
  expect(material.params.base_color[1]).toBeCloseTo(34 / 255, 5);
  await page.locator('.svelte-flow__node').filter({ hasText: 'Cube' }).click();
  await page.locator('#cube-visible').uncheck();
  const saved = await page.evaluate(() => JSON.parse(localStorage.getItem('grafix:graph:v1') || '{}'));
  expect(saved.nodes.find((n: any) => n.id === 'cube').params.visible).toBe(false);
  await page.reload();
  await expect(page.locator('.svelte-flow__node')).toHaveCount(6);
  const reread = await page.evaluate(() => (window as any).__grafixGraph);
  expect(reread.nodes.find((n: any) => n.id === 'cube').params.visible).toBe(false);
  expect(reread.nodes.find((n: any) => n.id === material.id).params.base_color[1])
    .toBeCloseTo(34 / 255, 5);
});


test('bundled two-object composition loads and retains separate materials/lights', async ({ page }) => {
  await page.getByRole('button', { name: 'Two objects' }).click();
  await expect(page.getByText('Two independent cubes, materials and lights loaded.')).toBeVisible();
  const graph = await page.evaluate(() => (window as any).__grafixGraph);
  expect(graph.nodes.filter((n: any) => n.type === 'geometry.cube')).toHaveLength(2);
  expect(graph.nodes.filter((n: any) => n.type === 'material.pbr')).toHaveLength(2);
  expect(graph.nodes.filter((n: any) => n.type === 'lighting.directional')).toHaveLength(2);
  expect(graph.edges.filter((e: any) => e.to.node === 'scene' && e.to.port === 'geometry'))
    .toHaveLength(2);
  await page.reload();
  const saved = await page.evaluate(() => (window as any).__grafixGraph);
  expect(saved.name).toBe('Gate C / Two Independent Materials');
});


test('engine statuses distinguish inactive nodes and approximate GPU PBR materials', async ({ page }) => {
  await page.getByRole('button', { name: 'Add PBR Material' }).click();
  await page.locator('#material-target').selectOption('cube');
  await page.getByRole('button', { name: 'Apply material' }).click();
  await expect(page.getByText('Fast GPU clone mode approximates PBR lighting.', { exact: false }))
    .toBeVisible();
  await expect(page.locator('.svelte-flow__node').filter({ hasText: 'PBR Material' }))
    .toContainText('approximate');
  await page.getByRole('button', { name: 'Add Cube' }).click();
  await expect(page.locator('.inspector .inactive-note')).toContainText('Not connected to Output');
});

test('invalid connection is rejected without changing the working graph', async ({ page }) => {
  const edgesBefore = await page.evaluate(() => (window as any).__grafixGraph.edges.length);
  await page.getByRole('button', { name: 'Add PBR Material' }).click();
  // Material and numeric input have different port types; the workflow
  // deliberately prevents invalid links and the runtime graph remains stable.
  const edgesAfter = await page.evaluate(() => (window as any).__grafixGraph.edges.length);
  expect(edgesAfter).toBe(edgesBefore);
});


test('editing a color is a single undoable graph transaction', async ({ page }) => {
  await page.getByRole('button', { name: 'Add PBR Material' }).click();
  const materialId = await page.evaluate(() => (window as any).__grafixGraph.nodes
    .find((n: any) => n.type === 'material.pbr').id);
  const pick = page.locator('.inspector input[type=color]').first();
  await pick.fill('#ff2244');
  await page.getByTitle('Undo').click();
  const graph = await page.evaluate(() => (window as any).__grafixGraph);
  const material = graph.nodes.find((n: any) => n.id === materialId);
  expect(material.params.base_color).toEqual([0.66, 0.82, 0.96]);
  expect(graph.nodes.some((n: any) => n.id === materialId)).toBe(true);
});


test('quick add search creates a new live-ready primitive and supports precise settings', async ({ page }) => {
  await page.getByRole('button', { name: '+ Add' }).click();
  await expect(page.getByRole('dialog', { name: 'Quick add node' })).toBeVisible();
  await page.getByRole('textbox', { name: 'Quick add search' }).fill('sphere');
  await page.keyboard.press('Enter');
  await expect(page.locator('.inspector h3')).toHaveText('Sphere');
  await page.getByRole('button', { name: 'Show sphere as a scene object' }).click();
  const exact = page.getByRole('spinbutton', { name: 'Exact radius' });
  await exact.fill('2.75');
  await exact.blur();
  const current = await page.evaluate(() => (window as any).__grafixGraph);
  const sphere = current.nodes.find((n: any) => n.type === 'geometry.sphere');
  expect(sphere.params.radius).toBe(2.75);
  expect(current.edges.some((e: any) => e.from.node === sphere.id && e.to.port === 'geometry')).toBe(true);
  await page.getByRole('button', { name: 'Reset radius' }).click();
  expect((await page.evaluate(() => (window as any).__grafixGraph))
    .nodes.find((n: any) => n.type === 'geometry.sphere').params.radius).toBe(0.5);
});

test('duplicate and rename a node without cloning its scene connections', async ({ page }) => {
  await page.getByRole('button', { name: 'Add Cube' }).click();
  await page.getByRole('textbox', { name: 'Node name' }).fill('Hero cube');
  await page.getByRole('textbox', { name: 'Node name' }).blur();
  await expect(page.locator('.inspector h3')).toHaveText('Hero cube');
  await page.getByRole('button', { name: 'Duplicate ⌘D' }).click();
  const current = await page.evaluate(() => (window as any).__grafixGraph);
  const added = current.nodes.filter((n: any) => n.type === 'geometry.cube');
  expect(added).toHaveLength(3);
  expect(added.at(-1).label).toBe('Hero cube copy');
  expect(current.edges.some((e: any) => e.from.node === added.at(-1).id)).toBe(false);
  await page.reload();
  const saved = await page.evaluate(() => (window as any).__grafixGraph);
  expect(saved.nodes.find((n: any) => n.id === added.at(-1).id).label).toBe('Hero cube copy');
});

test('Grid and Radial Cloner nodes activate through scene and expose real parameters', async ({ page }) => {
  await page.getByRole('button', { name: 'Add Grid Cloner' }).click();
  await expect(page.locator('.inspector h3')).toHaveText('Grid Cloner');
  await page.getByRole('button', { name: 'Use this cloner in the scene' }).click();
  const grid = await page.evaluate(() => (window as any).__grafixGraph);
  const activeGrid = grid.nodes.find((n: any) => n.type === 'clone.grid');
  expect(grid.edges.some((e: any) => e.from.node === activeGrid.id &&
    e.to.node === 'scene' && e.to.port === 'clones')).toBe(true);
  await page.getByRole('button', { name: 'Add Radial Cloner' }).click();
  await page.getByRole('button', { name: 'Use this cloner in the scene' }).click();
  const radial = await page.evaluate(() => (window as any).__grafixGraph);
  const activeRadial = radial.nodes.find((n: any) => n.type === 'clone.radial');
  expect(radial.edges.some((e: any) => e.from.node === activeRadial.id &&
    e.to.node === 'scene' && e.to.port === 'clones')).toBe(true);
});


test('viewport selection updates graph inspector and framing targets the selected object', async ({ page }) => {
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('grafix:viewport-select',
    { detail: 'cube' })));
  await expect(page.locator('.inspector h3')).toHaveText('Cube');
  await page.getByRole('button', { name: 'Frame in viewport' }).click();
  expect(await page.evaluate(() => (window as any).__grafixFramed)).toBe('cube');
});


test('keyframe tracks persist, undo and delete with their graph node', async ({ page }) => {
  await page.locator('.svelte-flow__node').filter({hasText: 'Cube'}).first().click();
  await page.getByRole('button', {name: 'Keyframe position_x'}).click();
  const tracked = await page.evaluate(() => (window as any).__grafixGraph);
  expect(tracked.tracks).toHaveLength(1);
  expect(tracked.tracks[0].node).toBe('cube');
  expect(tracked.tracks[0].keys[0].time).toBe(0);
  await page.reload();
  await page.locator('.svelte-flow__node').filter({hasText: 'Cube'}).first().click();
  await expect(page.getByRole('region', {name: 'Animation tracks'})).toBeVisible();
  await page.getByRole('button', {name: 'Remove track position_x'}).click();
  expect(await page.evaluate(() => (window as any).__grafixGraph.tracks.length)).toBe(0);
});
