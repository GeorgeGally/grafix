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
