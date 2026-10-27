// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later
import type { MountedFile } from './protocol';

/** The names along a path in the engine's file tree, or nothing when it could escape the tree. */
export function splitPath(path: string): string[] | undefined {
  const parts = path.split('/').filter(part => part && part !== '.');
  return parts.includes('..') || path.includes('\0') ? undefined : parts;
}

/** Where a host file is mounted. Host names must already be normal relative paths without backslashes. */
export function mountPath(input: MountedFile): string[] {
  const directory = input.directory ?? 'input';
  if (directory !== 'input' && directory !== 'identity') throw new Error('Invalid input directory');
  const parts = splitPath(input.name);
  if (!parts?.length || parts.join('/') !== input.name || input.name.includes('\\')) throw new Error('Invalid input filename');
  return [directory, ...parts];
}

export function validateMounts(files: MountedFile[]): void {
  const paths = new Set(files.map(input => mountPath(input).join('/')));
  if (paths.size !== files.length) throw new Error('Input filenames must be unique');
}
