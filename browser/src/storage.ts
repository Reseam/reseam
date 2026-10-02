// SPDX-FileCopyrightText: 2026 AunAli K. <hello@auna.li>
// SPDX-License-Identifier: GPL-3.0-or-later
import { Directory, Inode, OpenDirectory, Fd, wasi } from '@bjorn3/browser_wasi_shim';
import { Channel, decode, encode } from './channel';

class StorageFailure extends Error {
  constructor(message: string, readonly quota: boolean) { super(message); }
}
const errno = (error: unknown) => error instanceof StorageFailure && error.quota ? wasi.ERRNO_NOSPC : wasi.ERRNO_IO;

export class StorageClient {
  private readonly reads = new ReadCache<string>();
  constructor(readonly port: MessagePort, readonly channel: Channel) {}
  read(id: string, length: number, size: number, offset: number): Uint8Array {
    return this.reads.read(id, length, size, offset, (start, end) =>
      this.call('read', { id, offset: start, size: end - start }));
  }
  invalidate(id: string): void { this.reads.invalidate(id); }
  call<T>(operation: string, args: unknown): T {
    this.port.postMessage({ operation, args });
    const packet = this.channel.receiveSync();
    if (packet.kind !== 4) throw new Error('Unexpected storage reply');
    const response = decode<{ value?: T; error?: string; quota?: boolean }>(packet.bytes);
    if (response.error) throw new StorageFailure(response.error, !!response.quota);
    return response.value as T;
  }
}

export class DiskFile extends Inode {
  private length = 0n;
  private descriptors = 0;
  private links = 1;
  constructor(readonly storage: StorageClient, readonly id: string = crypto.randomUUID(), existingSize?: bigint) {
    super();
    if (existingSize === undefined) storage.call('create', { id });
    else this.length = existingSize;
  }
  writtenExternally(size: bigint): void {
    this.storage.invalidate(this.id);
    this.length = size;
  }
  get size(): bigint { return this.length; }
  stat(): wasi.Filestat { return new wasi.Filestat(this.ino, wasi.FILETYPE_REGULAR_FILE, this.size); }
  read(size: number, offset: bigint): Uint8Array {
    return this.storage.read(this.id, Number(this.length), size, Number(offset));
  }
  write(data: Uint8Array, offset: bigint): number {
    this.storage.invalidate(this.id);
    const count = this.storage.call<number>('write', { id: this.id, offset: Number(offset), data });
    this.length = this.length > offset + BigInt(count) ? this.length : offset + BigInt(count);
    return count;
  }
  truncate(size: bigint): void {
    this.storage.invalidate(this.id);
    this.storage.call('truncate', { id: this.id, size: Number(size) }); this.length = size;
  }
  sync(): void { this.storage.call('sync', { id: this.id }); }
  opened(): void { this.descriptors++; }
  closed(): void { this.descriptors--; this.removeUnused(); }
  link(): void { this.links++; }
  unlink(): void { this.links--; this.removeUnused(); }
  // Renames drop the old name before linking the new one, so detaching never deletes data.
  detach(): void { this.links--; }
  private removeUnused(): void {
    if (this.links === 0 && this.descriptors === 0) {
      this.storage.invalidate(this.id);
      this.storage.call('remove', { id: this.id });
    }
  }
  path_open(oflags: number, _rights: bigint, flags: number): { ret: number; fd_obj: Fd | null } {
    if (oflags & wasi.OFLAGS_TRUNC) this.truncate(0n);
    return { ret: 0, fd_obj: new DiskDescriptor(this, flags) };
  }
}

// ZIP readers repeatedly request nearby headers and short ranges. Each cache
// has an 8 MiB limit across files, including apps with many split APKs.
class ReadCache<K> {
  private pages: { key: K; offset: number; bytes: Uint8Array }[] = [];
  invalidate(key: K): void { this.pages = this.pages.filter(page => page.key !== key); }
  read(key: K, length: number, size: number, offset: number, fetch: (start: number, end: number) => Uint8Array): Uint8Array {
    const pageSize = 512 * 1024;
    const start = Math.floor(offset / pageSize) * pageSize;
    const end = Math.min(offset + size, length);
    if (offset >= end) return new Uint8Array();
    if (size > 128 * 1024 || end > start + pageSize) {
      return fetch(offset, end);
    }
    const index = this.pages.findIndex(page => page.key === key && page.offset === start);
    const page = index >= 0 ? this.pages.splice(index, 1)[0] : {
      key, offset: start, bytes: fetch(start, Math.min(start + pageSize, length)),
    };
    if (page.bytes.length !== Math.min(pageSize, length - start)) return fetch(offset, end);
    this.pages.unshift(page);
    if (this.pages.length > 16) this.pages.pop();
    return page.bytes.subarray(offset - start, end - start);
  }
}

export class InputReader {
  private readonly reader = new FileReaderSync();
  private readonly reads = new ReadCache<File>();
  read(file: File, size: number, offset: number): Uint8Array {
    return this.reads.read(file, file.size, size, offset, (start, end) =>
      new Uint8Array(this.reader.readAsArrayBuffer(file.slice(start, end))));
  }
}

// Input APKs remain browser File objects, so opening an APK does not copy it
// into OPFS or retain a second full-sized buffer.
export class InputFile extends Inode {
  constructor(readonly file: File, private readonly reader: InputReader) { super(); }
  get size(): bigint { return BigInt(this.file.size); }
  stat(): wasi.Filestat { return new wasi.Filestat(this.ino, wasi.FILETYPE_REGULAR_FILE, this.size); }
  read(size: number, offset: bigint): Uint8Array {
    return this.reader.read(this.file, size, Number(offset));
  }
  path_open(oflags: number, rights: bigint): { ret: number; fd_obj: Fd | null } {
    if (oflags & wasi.OFLAGS_TRUNC || rights & BigInt(wasi.RIGHTS_FD_WRITE)) return { ret: wasi.ERRNO_PERM, fd_obj: null };
    return { ret: 0, fd_obj: new DiskDescriptor(this, 0) };
  }
}

export class DiskDescriptor extends Fd {
  private position = 0n;
  private closed = false;
  constructor(readonly file: DiskFile | InputFile, private flags: number) { super(); if (file instanceof DiskFile) file.opened(); }
  fd_close() {
    if (this.closed) return wasi.ERRNO_BADF;
    this.closed = true;
    try { if (this.file instanceof DiskFile) this.file.closed(); return 0; } catch (error) { return errno(error); }
  }
  fd_fdstat_get() { return { ret: 0, fdstat: new wasi.Fdstat(wasi.FILETYPE_REGULAR_FILE, this.flags) }; }
  fd_filestat_get() { return { ret: 0, filestat: this.file.stat() }; }
  fd_tell() { return { ret: 0, offset: this.position }; }
  fd_seek(offset: bigint, whence: number) {
    const position = whence === wasi.WHENCE_SET ? offset : whence === wasi.WHENCE_CUR ? this.position + offset : whence === wasi.WHENCE_END ? this.file.size + offset : -1n;
    if (position < 0n) return { ret: wasi.ERRNO_INVAL, offset: 0n };
    this.position = position; return { ret: 0, offset: position };
  }
  fd_read(size: number) {
    const result = this.fd_pread(size, this.position); this.position += BigInt(result.data.length); return result;
  }
  fd_pread(size: number, offset: bigint) {
    try { return { ret: 0, data: this.file.read(size, offset) }; }
    catch (error) { return { ret: errno(error), data: new Uint8Array() }; }
  }
  fd_write(data: Uint8Array) {
    const offset = this.flags & wasi.FDFLAGS_APPEND ? this.file.size : this.position;
    const result = this.fd_pwrite(data, offset); this.position = offset + BigInt(result.nwritten); return result;
  }
  fd_pwrite(data: Uint8Array, offset: bigint) {
    if (!(this.file instanceof DiskFile)) return { ret: wasi.ERRNO_PERM, nwritten: 0 };
    try { return { ret: 0, nwritten: this.file.write(data, offset) }; }
    catch (error) { return { ret: errno(error), nwritten: 0 }; }
  }
  fd_filestat_set_size(size: bigint) {
    if (!(this.file instanceof DiskFile)) return wasi.ERRNO_PERM;
    try { this.file.truncate(size); return 0; } catch (error) { return errno(error); }
  }
  fd_allocate(offset: bigint, len: bigint) {
    return offset + len > this.file.size ? this.fd_filestat_set_size(offset + len) : 0;
  }
  fd_sync() {
    try { if (this.file instanceof DiskFile) this.file.sync(); return 0; } catch (error) { return errno(error); }
  }
  fd_filestat_set_times() { return 0; }
  fd_fdstat_set_flags(flags: number) { this.flags = flags; return 0; }
}

export class DiskDirectory extends Directory {
  constructor(readonly storage: StorageClient, contents = new Map<string, Inode>()) { super(contents); }
  override path_open() { return { ret: 0, fd_obj: new DiskDirectoryDescriptor(this) }; }
  override create_entry_for_path(path: string, isDirectory: boolean) {
    const parts = path.split('/').filter(part => part && part !== '.');
    if (path.startsWith('/') || parts.includes('..') || path.includes('\0')) return { ret: wasi.ERRNO_NOTCAPABLE, entry: null };
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
