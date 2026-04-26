#pragma once

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdatomic.h>

typedef struct FfiStatus { int32_t code; } FfiStatus;
typedef struct FfiString { uint8_t* ptr; size_t len; size_t cap; } FfiString;
typedef struct FfiBuf_u8 { uint8_t* ptr; size_t len; size_t cap; size_t align; } FfiBuf_u8;
typedef struct FfiError { FfiString message; } FfiError;
typedef struct BoltFFICallbackHandle { uint64_t handle; const void* vtable; } BoltFFICallbackHandle;

static inline bool boltffi_atomic_u8_cas(volatile uint8_t* target, uint8_t expected, uint8_t desired) {
    return atomic_compare_exchange_strong((_Atomic uint8_t*)target, &expected, desired);
}

static inline uint64_t boltffi_atomic_u64_exchange(volatile uint64_t* target, uint64_t value) {
    return atomic_exchange((_Atomic uint64_t*)target, value);
}

static inline bool boltffi_atomic_u64_cas(volatile uint64_t* target, uint64_t expected, uint64_t desired) {
    return atomic_compare_exchange_strong((_Atomic uint64_t*)target, &expected, desired);
}

static inline uint64_t boltffi_atomic_u64_load(const volatile uint64_t* target) {
    return atomic_load_explicit((const _Atomic uint64_t*)target, memory_order_acquire);
}


typedef struct PatchEventSinkVTable {
    void (*free)(uint64_t handle);
    uint64_t (*clone)(uint64_t handle);
    void (*on_event)(uint64_t handle, const uint8_t* event_json, uintptr_t event_json_len, FfiStatus *_out_status);
} PatchEventSinkVTable;

void boltffi_register_patch_event_sink_vtable(const struct PatchEventSinkVTable *vtable);
BoltFFICallbackHandle boltffi_create_patch_event_sink_handle(uint64_t handle);
FfiBuf_u8 boltffi_inspect_apk_json(const uint8_t* apk_path, uintptr_t apk_path_len, const uint8_t* split_paths_json, uintptr_t split_paths_json_len);FfiBuf_u8 boltffi_inspect_json(const uint8_t* request_json, uintptr_t request_json_len);FfiBuf_u8 boltffi_patch_json(const uint8_t* request_json, uintptr_t request_json_len, BoltFFICallbackHandle event_sink);
void boltffi_free_string(FfiString s);
void boltffi_free_buf(FfiBuf_u8 buf);
FfiStatus boltffi_last_error_message(FfiString *out);
void boltffi_clear_last_error(void);
