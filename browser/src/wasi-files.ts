// SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { Inode, Fd, wasi } from '@bjorn3/browser_wasi_shim';
import { type InputReader, type StorageClient, StorageFailure } from './storage-client';

export const errno = (error: unknown) => error instanceof StorageFailure && error.quota ? wasi.ERRNO_NOSPC : wasi.ERRNO_IO;

/** A scratch file in OPFS, deleted once no name or descriptor refers to it. */
export class DiskFile extends Inode {
  private length = 0n;
  private descriptors = 0;
  private links = 1;
  constructor(readonly storage: StorageClient, readonly id: string = crypto.randomUUID(), existingSize?: bigint) {
    super();
    if (existingSize === undefined) storage.call({ operation: 'create', id });
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
    const count = this.storage.call<number>({ operation: 'write', id: this.id, offset: Number(offset), data });
    this.length = this.length > offset + BigInt(count) ? this.length : offset + BigInt(count);
    return count;
  }
  truncate(size: bigint): void {
    this.storage.invalidate(this.id);
    this.storage.call({ operation: 'truncate', id: this.id, size: Number(size) }); this.length = size;
  }
  sync(): void { this.storage.call({ operation: 'sync', id: this.id }); }
  opened(): void { this.descriptors++; }
  closed(): void { this.descriptors--; this.removeUnused(); }
  link(): void { this.links++; }
  unlink(): void { this.links--; this.removeUnused(); }
  // Renames drop the old name before linking the new one, so detaching never deletes data.
  detach(): void { this.links--; }
  private removeUnused(): void {
    if (this.links === 0 && this.descriptors === 0) {
      this.storage.invalidate(this.id);
      this.storage.call({ operation: 'remove', id: this.id });
    }
  }
  path_open(oflags: number, _rights: bigint, flags: number): { ret: number; fd_obj: Fd | null } {
    if (oflags & wasi.OFLAGS_TRUNC) this.truncate(0n);
    return { ret: 0, fd_obj: new DiskDescriptor(this, flags) };
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

