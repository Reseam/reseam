// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { Directory, Inode, OpenDirectory, wasi } from '@bjorn3/browser_wasi_shim';
import { splitPath } from './paths';
import type { StorageClient } from './storage-client';
import { DiskFile, errno } from './wasi-files';

export class DiskDirectory extends Directory {
  constructor(readonly storage: StorageClient, contents = new Map<string, Inode>()) { super(contents); }
  override path_open() { return { ret: 0, fd_obj: new DiskDirectoryDescriptor(this) }; }
  override create_entry_for_path(path: string, isDirectory: boolean) {
    const parts = splitPath(path);
    if (!parts || path.startsWith('/')) return { ret: wasi.ERRNO_NOTCAPABLE, entry: null };
    const name = parts.pop();
    if (!name) return { ret: wasi.ERRNO_INVAL, entry: null };
    let parent: Directory = this;
    for (const part of parts) {
      const child = parent.contents.get(part);
      if (!(child instanceof Directory)) return { ret: wasi.ERRNO_NOENT, entry: null };
      parent = child;
    }
    if (parent.contents.has(name)) return { ret: wasi.ERRNO_EXIST, entry: null };
    try {
      const entry = isDirectory ? new DiskDirectory(this.storage) : new DiskFile(this.storage);
      parent.contents.set(name, entry); return { ret: 0, entry };
    } catch (error) { return { ret: errno(error), entry: null }; }
  }
}

class DiskDirectoryDescriptor extends OpenDirectory {
  override path_link(path: string, inode: Inode, allowDirectory: boolean): number {
    const { inode_obj: replaced } = this.path_lookup(path, 0);
    const result = super.path_link(path, inode, allowDirectory);
    if (result !== 0) return result;
    if (inode instanceof DiskFile) inode.link();
    if (replaced instanceof DiskFile && replaced !== inode) {
      try { replaced.unlink(); } catch (error) { return errno(error); }
    }
    return result;
  }
  override path_unlink(path: string): { ret: number; inode_obj: Inode | null } {
    const result = super.path_unlink(path);
    if (result.inode_obj instanceof DiskFile) result.inode_obj.detach();
    return result;
  }
  override path_unlink_file(path: string): number {
    const { inode_obj: inode } = this.path_lookup(path, 0);
    const result = super.path_unlink_file(path);
    if (result === 0 && inode instanceof DiskFile) {
      try { inode.unlink(); } catch (error) { return errno(error); }
    }
    return result;
  }
}

export class RootDescriptor extends DiskDirectoryDescriptor {
  fd_prestat_get() { return { ret: 0, prestat: wasi.Prestat.dir('/') }; }
}

