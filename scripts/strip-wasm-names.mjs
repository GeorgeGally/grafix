// Removes the custom `name` section from a wasm module in place.
// Usage: node scripts/strip-wasm-names.mjs <file.wasm>
import { readFileSync, writeFileSync } from 'node:fs';

const file = process.argv[2];
if (!file) {
  console.error('usage: strip-wasm-names.mjs <file.wasm>');
  process.exit(2);
}
const bytes = readFileSync(file);
if (bytes.readUInt32LE(0) !== 0x6d736100) throw new Error(`${file} is not a wasm module`);

function leb(offset) {
  let value = 0;
  let shift = 0;
  for (;;) {
    const byte = bytes[offset++];
    value += (byte & 0x7f) * 2 ** shift;
    shift += 7;
    if (byte < 0x80) return [value, offset];
  }
}

const kept = [bytes.subarray(0, 8)];
let removed = 0;
let offset = 8;
while (offset < bytes.length) {
  const start = offset;
  const id = bytes[offset++];
  const [size, body] = leb(offset);
  const end = body + size;
  let isName = false;
  if (id === 0) {
    const [nameLength, nameStart] = leb(body);
    isName = bytes.toString('utf8', nameStart, nameStart + nameLength) === 'name';
  }
  if (isName) removed += end - start;
  else kept.push(bytes.subarray(start, end));
  offset = end;
}
writeFileSync(file, Buffer.concat(kept));
console.log(`removed name section: ${(removed / 2 ** 20).toFixed(1)} MiB`);
