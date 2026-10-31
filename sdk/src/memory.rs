// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

// Current RSS can fall after collection and cache purging; process high-water
// counters deliberately remain unchanged.
pub(crate) fn release_process_memory() {
    reseam_patcher::release_runtime_memory();
    purge_native_heap();
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn purge_native_heap() {
    // SAFETY: malloc_trim only releases free memory held by the allocator.
    unsafe {
        libc::malloc_trim(0);
    }
}

#[cfg(target_os = "android")]
fn purge_native_heap() {
    const M_PURGE: libc::c_int = -101;
    unsafe extern "C" {
        fn mallopt(param: libc::c_int, value: libc::c_int) -> libc::c_int;
    }
    // SAFETY: M_PURGE asks scudo to release cached free pages; it touches no live allocation.
    unsafe {
        mallopt(M_PURGE, 0);
    }
}

#[cfg(not(any(all(target_os = "linux", target_env = "gnu"), target_os = "android")))]
fn purge_native_heap() {}
