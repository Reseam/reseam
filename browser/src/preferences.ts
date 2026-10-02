// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later
export interface Identity { key: ArrayBuffer; cert: ArrayBuffer }
let database: Promise<IDBDatabase> | undefined;
function open(): Promise<IDBDatabase> {
  return database ??= new Promise((resolve, reject) => {
    const request = indexedDB.open('reseam-browser', 1);
    request.onupgradeneeded = () => request.result.createObjectStore('preferences');
    request.onsuccess = () => { request.result.onversionchange = () => { request.result.close(); database = undefined; }; resolve(request.result); };
    request.onerror = () => { database = undefined; reject(request.error); };
    request.onblocked = () => reject(new Error('Close another Reseam tab to update browser storage'));
  });
}
export async function preference<T>(key: 'identity' | 'trusted-signers'): Promise<T | undefined> {
  const db = await open();
  return await new Promise((resolve, reject) => {
    const transaction = db.transaction('preferences', 'readonly');
    const request = transaction.objectStore('preferences').get(key);
    request.onsuccess = () => resolve(request.result as T | undefined);
    request.onerror = () => reject(request.error);
  });
}
export async function savePreference(key: 'identity' | 'trusted-signers', value: unknown): Promise<void> {
  const db = await open();
  await new Promise<void>((resolve, reject) => {
    const transaction = db.transaction('preferences', 'readwrite', { durability: 'strict' });
    transaction.objectStore('preferences').put(value, key);
    transaction.oncomplete = () => resolve();
    transaction.onabort = () => reject(transaction.error ?? new Error('Browser settings were not saved'));
    transaction.onerror = () => reject(transaction.error);
  });
}
