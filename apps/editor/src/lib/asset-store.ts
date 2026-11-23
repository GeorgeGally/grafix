/**
 * Local, persistent GLB library. Project JSON keeps content-hash references,
 * never transient object URLs or machine-specific filesystem paths.
 */
const DB_NAME = 'grafix-assets-v1';
const STORE = 'glb';

function database(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, 1);
    request.onupgradeneeded = () => {
      if (!request.result.objectStoreNames.contains(STORE))
        request.result.createObjectStore(STORE);
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error('Asset storage unavailable'));
  });
}

export async function glbHash(bytes: Uint8Array): Promise<string> {
  const buffer = new ArrayBuffer(bytes.byteLength);
  new Uint8Array(buffer).set(bytes);
  const digest = await crypto.subtle.digest('SHA-256', buffer);
  return [...new Uint8Array(digest)].map(b => b.toString(16).padStart(2, '0')).join('');
}

export function checkGlb(bytes: Uint8Array): void {
  if (bytes.byteLength < 20 || bytes.byteLength > 50_000_000)
    throw new Error('GLB must be between 20 bytes and 50 MB');
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (view.getUint32(0, true) !== 0x46546c67 || view.getUint32(4, true) !== 2 ||
    view.getUint32(8, true) !== bytes.byteLength)
    throw new Error('Invalid binary glTF 2.0 file');
}

export async function storeGlb(id: string, bytes: Uint8Array): Promise<void> {
  const db = await database();
  try {
    await new Promise<void>((resolve, reject) => {
      const transaction = db.transaction(STORE, 'readwrite');
      transaction.objectStore(STORE).put(bytes, id);
      transaction.oncomplete = () => resolve();
      transaction.onerror = () => reject(transaction.error ?? new Error('Asset write failed'));
      transaction.onabort = () => reject(transaction.error ?? new Error('Asset storage aborted'));
    });
  } finally { db.close(); }
}

export async function readGlb(id: string): Promise<Uint8Array | null> {
  const db = await database();
  try {
    return await new Promise<Uint8Array | null>((resolve, reject) => {
      const transaction = db.transaction(STORE, 'readonly');
      const request = transaction.objectStore(STORE).get(id);
      request.onsuccess = () => {
        const value = request.result;
        resolve(value ? new Uint8Array(value) : null);
      };
      request.onerror = () => reject(request.error ?? new Error('Asset read failed'));
    });
  } finally { db.close(); }
}
