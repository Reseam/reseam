// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { Directory, Inode, Fd } from '@bjorn3/browser_wasi_shim';
import { mountPath, splitPath } from './paths';
import type { MountedFile, StoredFile } from './protocol';
import { InputReader, type StorageClient } from './storage-client';
import { DiskDirectory, RootDescriptor } from './wasi-directories';
import { DiskFile, InputFile } from './wasi-files';

/** The engine's file tree: mounted host files, scratch space, outputs and signing identity. */
export class Filesystem {
  private readonly root: DiskDirectory;
  private readonly inputs = new InputReader();
  constructor(private readonly storage: StorageClient) {
    this.root = new DiskDirectory(storage);
    for (const name of ['input', 'tmp', 'output', 'identity']) this.root.contents.set(name, new DiskDirectory(storage));
  }
  get descriptor(): Fd { return new RootDescriptor(this.root); }
  lookup(path: string): Inode {
    const parts = splitPath(path);
    if (!parts) throw new Error('Invalid browser file path');
    let inode: Inode | undefined = this.root;
    for (const part of parts) {
      if (!(inode instanceof Directory)) throw new Error(`No directory in ${path}`);
      inode = inode.contents.get(part);
    }
    if (!inode) throw new Error(`No browser file ${path}`);
    return inode;
  }
  readFile(path: string): Uint8Array {
    const file = this.lookup(path);
    if (!(file instanceof DiskFile || file instanceof InputFile)) throw new Error(`Not a file: ${path}`);
    const length = Number(file.size);
    const data = new Uint8Array(length);
    for (let offset = 0; offset < length;) {
      const bytes = file.read(Math.min(1 << 20, length - offset), BigInt(offset));
      if (!bytes.length) throw new Error(`Unexpected end of ${path}`);
      data.set(bytes, offset); offset += bytes.length;
    }
    return data;
  }
  mount(files: MountedFile[]): void {
    for (const input of files) {
      const parts = mountPath(input);
      const name = parts.pop()!;
      let folder: Directory = this.root;
      for (const part of parts) {
        let child = folder.contents.get(part);
        if (!child) { child = new DiskDirectory(this.storage); folder.contents.set(part, child); }
        if (!(child instanceof Directory)) throw new Error('Input path collides with a file');
        folder = child;
      }
      if (folder.contents.has(name)) throw new Error('Duplicate input path');
      folder.contents.set(name, new InputFile(input.file, this.inputs));
    }
  }
  /** Every file under `output/`, and the signing identity the engine may have generated. */
  artifacts(): StoredFile[] {
    const files: StoredFile[] = [];
    const add = (name: string, inode: Inode) => { if (inode instanceof DiskFile) files.push({ name, storageId: inode.id }); };
    const walk = (directory: Directory, prefix: string) => {
      for (const [name, inode] of directory.contents) {
        if (inode instanceof Directory) walk(inode, `${prefix}${name}/`); else add(prefix + name, inode);
      }
    };
    walk(this.directory('output'), '');
    for (const [name, inode] of this.directory('identity').contents) add(`identity/${name}`, inode);
    return files;
  }
  private directory(name: string): Directory {
    const inode = this.root.contents.get(name);
    if (!(inode instanceof Directory)) throw new Error(`The ${name} directory is missing`);
    return inode;
  }
}
