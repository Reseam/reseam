#include <jni.h>
#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>
#include <stdatomic.h>
#include <reseam-sdk.h>

static inline bool boltffi_exception_pending(JNIEnv* env) {
    return (*env)->ExceptionCheck(env);
}

static inline bool boltffi_consume_pending_exception(JNIEnv* env) {
    if (!boltffi_exception_pending(env)) return false;
    (*env)->ExceptionClear(env);
    return true;
}

static inline void boltffi_throw_out_of_memory(JNIEnv* env, const char* message) {
    jclass oom_class = (*env)->FindClass(env, "java/lang/OutOfMemoryError");
    if (oom_class == NULL) return;
    (*env)->ThrowNew(env, oom_class, message);
    (*env)->DeleteLocalRef(env, oom_class);
}

static inline void boltffi_throw_illegal_argument(JNIEnv* env, const char* message) {
    jclass exception_class = (*env)->FindClass(env, "java/lang/IllegalArgumentException");
    if (exception_class == NULL) return;
    (*env)->ThrowNew(env, exception_class, message);
    (*env)->DeleteLocalRef(env, exception_class);
}

static inline void boltffi_throw_runtime(JNIEnv* env, const char* message) {
    jclass exception_class = (*env)->FindClass(env, "java/lang/RuntimeException");
    if (exception_class == NULL) return;
    (*env)->ThrowNew(env, exception_class, message);
    (*env)->DeleteLocalRef(env, exception_class);
}

static inline void boltffi_throw_status(JNIEnv* env, FfiStatus status, const char* fallback_message) {
    if (status.code == 3) {
        boltffi_throw_illegal_argument(env, "invalid argument");
    } else if (status.code == 4) {
        boltffi_throw_runtime(env, "operation cancelled");
    } else {
        boltffi_throw_runtime(env, fallback_message);
    }
}

static inline bool boltffi_try_jlong_to_usize(jlong value, uintptr_t* out_value) {
    if (value < 0) return false;
    uint64_t unsigned_value = (uint64_t)value;
    if (unsigned_value > (uint64_t)UINTPTR_MAX) return false;
    *out_value = (uintptr_t)unsigned_value;
    return true;
}

typedef struct {
    void (*free)(uint64_t handle);
    uint64_t (*clone)(uint64_t handle);
} BoltFFICallbackVTablePrefix;

static inline const BoltFFICallbackVTablePrefix* boltffi_callback_vtable_prefix(
    const BoltFFICallbackHandle* callback
) {
    return callback == NULL ? NULL : (const BoltFFICallbackVTablePrefix*)callback->vtable;
}

static inline void boltffi_release_callback_value(BoltFFICallbackHandle callback) {
    const BoltFFICallbackVTablePrefix* vtable = boltffi_callback_vtable_prefix(&callback);
    if (callback.handle != 0 && vtable != NULL && vtable->free != NULL) {
        vtable->free(callback.handle);
    }
}

static inline BoltFFICallbackHandle* boltffi_jvm_callback_handle_ref(jlong handle) {
    if (handle == 0) return NULL;
    return (BoltFFICallbackHandle*)(uintptr_t)handle;
}

static inline jlong boltffi_jvm_callback_handle_new_owned(
    JNIEnv* env,
    BoltFFICallbackHandle callback
) {
    if (callback.handle == 0 || callback.vtable == NULL) return 0;
    BoltFFICallbackHandle* stored_callback =
        (BoltFFICallbackHandle*)malloc(sizeof(BoltFFICallbackHandle));
    if (stored_callback == NULL) {
        boltffi_release_callback_value(callback);
        boltffi_throw_out_of_memory(env, "Failed to allocate callback handle");
        return 0;
    }
    *stored_callback = callback;
    return (jlong)(uintptr_t)stored_callback;
}

static inline void boltffi_jvm_callback_handle_release(BoltFFICallbackHandle* callback) {
    if (callback == NULL) return;
    boltffi_release_callback_value(*callback);
    free(callback);
}

static inline jlong boltffi_jvm_callback_handle_clone(
    JNIEnv* env,
    const BoltFFICallbackHandle* callback
) {
    const BoltFFICallbackVTablePrefix* vtable = boltffi_callback_vtable_prefix(callback);
    if (callback == NULL || callback->handle == 0 || vtable == NULL || vtable->clone == NULL) {
        return 0;
    }
    BoltFFICallbackHandle cloned_callback = {
        .handle = vtable->clone(callback->handle),
        .vtable = callback->vtable,
    };
    if (cloned_callback.handle == 0) {
        return 0;
    }
    return boltffi_jvm_callback_handle_new_owned(env, cloned_callback);
}

static inline jbyteArray boltffi_buf_to_jbytearray(JNIEnv* env, FfiBuf_u8 buf) {
    if (buf.ptr == NULL) {
        if (buf.len != 0) {
            boltffi_throw_runtime(env, "BoltFFI buffer pointer was null with non-zero length");
        }
        return NULL;
    }
    if (buf.len > (size_t)INT32_MAX) {
        boltffi_free_buf(buf);
        boltffi_throw_out_of_memory(env, "BoltFFI buffer too large for Java byte array");
        return NULL;
    }
    jsize len = (jsize)buf.len;
    jbyteArray arr = (*env)->NewByteArray(env, len);
    if (arr == NULL) {
        boltffi_free_buf(buf);
        return NULL;
    }
    (*env)->SetByteArrayRegion(env, arr, 0, len, (const jbyte*)buf.ptr);
    boltffi_free_buf(buf);
    if (boltffi_exception_pending(env)) {
        (*env)->DeleteLocalRef(env, arr);
        return NULL;
    }
    return arr;
}

static inline jbyteArray boltffi_status_buf_to_jbytearray(JNIEnv* env, FfiStatus status, FfiBuf_u8 buf) {
    if (status.code != 0) {
        if (buf.ptr != NULL) {
            boltffi_free_buf(buf);
        }
        boltffi_throw_status(env, status, "ffi call failed");
        return NULL;
    }
    return boltffi_buf_to_jbytearray(env, buf);
}

static inline uint32_t boltffi_le_u32(const uint8_t* bytes) {
    return
        ((uint32_t)bytes[0]) |
        ((uint32_t)bytes[1] << 8) |
        ((uint32_t)bytes[2] << 16) |
        ((uint32_t)bytes[3] << 24);
}

static inline jstring boltffi_utf8_buf_to_jstring(JNIEnv* env, FfiBuf_u8 buf) {
    if (buf.ptr == NULL) {
        if (buf.len != 0) {
            boltffi_throw_runtime(env, "BoltFFI string buffer pointer was null with non-zero length");
        }
        return NULL;
    }
    if (buf.len < 4) {
        boltffi_free_buf(buf);
        boltffi_throw_runtime(env, "BoltFFI string buffer missing length prefix");
        return NULL;
    }

    const uint8_t* bytes = (const uint8_t*)buf.ptr;
    size_t payload_len = (size_t)boltffi_le_u32(bytes);
    if (payload_len > buf.len - 4) {
        boltffi_free_buf(buf);
        boltffi_throw_runtime(env, "BoltFFI string buffer length prefix exceeded payload");
        return NULL;
    }
    if (payload_len == 0) {
        boltffi_free_buf(buf);
        return (*env)->NewString(env, NULL, 0);
    }
    if (payload_len > (size_t)INT32_MAX) {
        boltffi_free_buf(buf);
        boltffi_throw_out_of_memory(env, "BoltFFI string too large for Java string");
        return NULL;
    }

    const uint8_t* utf8 = bytes + 4;
    jchar stack_chars[64];
    jchar* chars = stack_chars;
    if (payload_len > sizeof(stack_chars) / sizeof(stack_chars[0])) {
        chars = (jchar*)malloc(payload_len * sizeof(jchar));
        if (chars == NULL) {
            boltffi_free_buf(buf);
            boltffi_throw_out_of_memory(env, "Failed to allocate Java string buffer");
            return NULL;
        }
    }

    size_t in_pos = 0;
    size_t out_pos = 0;
    bool invalid_utf8 = false;
    while (in_pos < payload_len) {
        uint8_t b0 = utf8[in_pos];
        if (b0 < 0x80) {
            chars[out_pos++] = (jchar)b0;
            in_pos += 1;
            continue;
        }

        uint32_t codepoint = 0;
        if ((b0 & 0xE0) == 0xC0) {
            if (in_pos + 1 >= payload_len) {
                invalid_utf8 = true;
                break;
            }
            uint8_t b1 = utf8[in_pos + 1];
            if ((b1 & 0xC0) != 0x80) {
                invalid_utf8 = true;
                break;
            }
            codepoint = ((uint32_t)(b0 & 0x1F) << 6) | (uint32_t)(b1 & 0x3F);
            if (codepoint < 0x80) {
                invalid_utf8 = true;
                break;
            }
            chars[out_pos++] = (jchar)codepoint;
            in_pos += 2;
            continue;
        }

        if ((b0 & 0xF0) == 0xE0) {
            if (in_pos + 2 >= payload_len) {
                invalid_utf8 = true;
                break;
            }
            uint8_t b1 = utf8[in_pos + 1];
            uint8_t b2 = utf8[in_pos + 2];
            if ((b1 & 0xC0) != 0x80 || (b2 & 0xC0) != 0x80) {
                invalid_utf8 = true;
                break;
            }
            codepoint =
                ((uint32_t)(b0 & 0x0F) << 12) |
                ((uint32_t)(b1 & 0x3F) << 6) |
                (uint32_t)(b2 & 0x3F);
            if (codepoint < 0x800 || (codepoint >= 0xD800 && codepoint <= 0xDFFF)) {
                invalid_utf8 = true;
                break;
            }
            chars[out_pos++] = (jchar)codepoint;
            in_pos += 3;
            continue;
        }

        if ((b0 & 0xF8) == 0xF0) {
            if (in_pos + 3 >= payload_len) {
                invalid_utf8 = true;
                break;
            }
            uint8_t b1 = utf8[in_pos + 1];
            uint8_t b2 = utf8[in_pos + 2];
            uint8_t b3 = utf8[in_pos + 3];
            if ((b1 & 0xC0) != 0x80 || (b2 & 0xC0) != 0x80 || (b3 & 0xC0) != 0x80) {
                invalid_utf8 = true;
                break;
            }
            codepoint =
                ((uint32_t)(b0 & 0x07) << 18) |
                ((uint32_t)(b1 & 0x3F) << 12) |
                ((uint32_t)(b2 & 0x3F) << 6) |
                (uint32_t)(b3 & 0x3F);
            if (codepoint < 0x10000 || codepoint > 0x10FFFF) {
                invalid_utf8 = true;
                break;
            }
            uint32_t surrogate = codepoint - 0x10000;
            chars[out_pos++] = (jchar)(0xD800 + (surrogate >> 10));
            chars[out_pos++] = (jchar)(0xDC00 + (surrogate & 0x3FF));
            in_pos += 4;
            continue;
        }

        invalid_utf8 = true;
        break;
    }

    jstring result = NULL;
    if (!invalid_utf8) {
        if (out_pos > (size_t)INT32_MAX) {
            boltffi_throw_out_of_memory(env, "BoltFFI string too large for Java string");
        } else {
            result = (*env)->NewString(env, chars, (jsize)out_pos);
        }
    }

    if (chars != stack_chars) {
        free(chars);
    }
    boltffi_free_buf(buf);

    if (invalid_utf8) {
        char message[96];
        snprintf(
            message,
            sizeof(message),
            "BoltFFI string buffer contained invalid UTF-8 at byte offset %zu",
            in_pos
        );
        boltffi_throw_runtime(env, message);
        return NULL;
    }
    return result;
}

static inline bool boltffi_lookup_static_method(
    JNIEnv* env,
    jclass cls,
    const char* name,
    const char* signature,
    jmethodID* out_method
) {
    *out_method = (*env)->GetStaticMethodID(env, cls, name, signature);
    if (*out_method != NULL) return true;
    boltffi_consume_pending_exception(env);
    return false;
}

typedef enum {
    BOLTFFI_GLOBAL_CLASS_OK = 0,
    BOLTFFI_GLOBAL_CLASS_MISSING = 1,
    BOLTFFI_GLOBAL_CLASS_FATAL = 2
} BoltFFIGlobalClassResult;

static inline BoltFFIGlobalClassResult boltffi_lookup_global_class(
    JNIEnv* env,
    const char* class_name,
    jclass* out_class
) {
    *out_class = NULL;
    jclass local_class = (*env)->FindClass(env, class_name);
    if (local_class == NULL) {
        boltffi_consume_pending_exception(env);
        return BOLTFFI_GLOBAL_CLASS_MISSING;
    }
    jclass global_class = (*env)->NewGlobalRef(env, local_class);
    (*env)->DeleteLocalRef(env, local_class);
    if (global_class == NULL) {
        boltffi_consume_pending_exception(env);
        return BOLTFFI_GLOBAL_CLASS_FATAL;
    }
    *out_class = global_class;
    return BOLTFFI_GLOBAL_CLASS_OK;
}

typedef enum {
    BOLTFFI_STATIC_CALL_CACHE_UNINIT = 0,
    BOLTFFI_STATIC_CALL_CACHE_INITING = 1,
    BOLTFFI_STATIC_CALL_CACHE_READY = 2,
    BOLTFFI_STATIC_CALL_CACHE_FAILED = 3
} BoltFFIStaticCallCacheState;

typedef struct {
    atomic_int state;
    jclass class_ref;
    jmethodID method;
} BoltFFIStaticCallCache;

#define BOLTFFI_STATIC_CALL_CACHE_INIT { 0, NULL, NULL }

static inline bool boltffi_static_call_cache_ensure(
    JNIEnv* env,
    BoltFFIStaticCallCache* cache,
    const char* class_name,
    const char* method_name,
    const char* method_signature
) {
    int state = atomic_load_explicit(&cache->state, memory_order_acquire);
    if (state == BOLTFFI_STATIC_CALL_CACHE_READY) return true;
    if (state == BOLTFFI_STATIC_CALL_CACHE_FAILED) return false;

    int expected = BOLTFFI_STATIC_CALL_CACHE_UNINIT;
    if (atomic_compare_exchange_strong_explicit(
            &cache->state,
            &expected,
            BOLTFFI_STATIC_CALL_CACHE_INITING,
            memory_order_acq_rel,
            memory_order_acquire)) {
        jclass class_ref = NULL;
        jmethodID method = NULL;
        BoltFFIGlobalClassResult class_result =
            boltffi_lookup_global_class(env, class_name, &class_ref);
        if (class_result != BOLTFFI_GLOBAL_CLASS_OK) {
            cache->class_ref = NULL;
            cache->method = NULL;
            atomic_store_explicit(
                &cache->state,
                BOLTFFI_STATIC_CALL_CACHE_FAILED,
                memory_order_release
            );
            return false;
        }
        if (!boltffi_lookup_static_method(env, class_ref, method_name, method_signature, &method)) {
            (*env)->DeleteGlobalRef(env, class_ref);
            cache->class_ref = NULL;
            cache->method = NULL;
            atomic_store_explicit(
                &cache->state,
                BOLTFFI_STATIC_CALL_CACHE_FAILED,
                memory_order_release
            );
            return false;
        }
        cache->class_ref = class_ref;
        cache->method = method;
        atomic_store_explicit(
            &cache->state,
            BOLTFFI_STATIC_CALL_CACHE_READY,
            memory_order_release
        );
        return true;
    }

    do {
        state = atomic_load_explicit(&cache->state, memory_order_acquire);
    } while (state == BOLTFFI_STATIC_CALL_CACHE_INITING);

    return state == BOLTFFI_STATIC_CALL_CACHE_READY;
}

static inline void boltffi_static_call_cache_reset(JNIEnv* env, BoltFFIStaticCallCache* cache) {
    if (cache->class_ref != NULL) {
        (*env)->DeleteGlobalRef(env, cache->class_ref);
        cache->class_ref = NULL;
    }
    cache->method = NULL;
    atomic_store_explicit(&cache->state, BOLTFFI_STATIC_CALL_CACHE_UNINIT, memory_order_release);
}
static JavaVM* g_jvm = NULL;
static jclass g_callback_class = NULL;
static jmethodID g_callback_method = NULL;
typedef enum {
    BOLTFFI_CALLBACK_INIT_OK = 0,
    BOLTFFI_CALLBACK_INIT_SKIPPED = 1,
    BOLTFFI_CALLBACK_INIT_FATAL = 2
} BoltFFICallbackInitResult;
static jint boltffi_attach_current_thread(JavaVM* vm, JNIEnv** env) {
#if defined(__ANDROID__)
    return (*vm)->AttachCurrentThread(vm, env, NULL);
#else
    return (*vm)->AttachCurrentThread(vm, (void**)env, NULL);
#endif
}
static BoltFFICallbackInitResult init_PatchEventSink_callbacks(JNIEnv* env);

JNIEXPORT jint JNICALL JNI_OnLoad(JavaVM* vm, void* reserved) {
    g_jvm = vm;
    JNIEnv* env;
    if ((*vm)->GetEnv(vm, (void**)&env, JNI_VERSION_1_6) != JNI_OK) {
        return JNI_ERR;
    }
    /*
     * BoltFFI 0.24.1 emits this lookup whenever a callback trait exists, even
     * when the module has only synchronous callbacks. Kotlin correctly omits
     * boltffiFutureContinuationCallback unless async functions/streams/async
     * callbacks exist, so keep JNI_OnLoad limited to the sync callback setup.
     * Remove this once BoltFFI gates the JNI continuation lookup the same way
     * as the Kotlin Native template.
     */
    if (init_PatchEventSink_callbacks(env) == BOLTFFI_CALLBACK_INIT_FATAL) {
        return JNI_ERR;
    }
    return JNI_VERSION_1_6;
}

static void boltffi_jni_continuation_callback(uint64_t handle, int8_t poll_result) {
    JNIEnv* env;
    int attached = 0;
    jint get_env_result = (*g_jvm)->GetEnv(g_jvm, (void**)&env, JNI_VERSION_1_6);
    if (get_env_result == JNI_EDETACHED) {
        if (boltffi_attach_current_thread(g_jvm, &env) != JNI_OK) {
            return;
        }
        attached = 1;
    } else if (get_env_result != JNI_OK) {
        return;
    }
    (*env)->CallStaticVoidMethod(env, g_callback_class, g_callback_method, (jlong)handle, (jbyte)poll_result);
    boltffi_consume_pending_exception(env);
    if (attached) {
        (*g_jvm)->DetachCurrentThread(g_jvm);
    }
}
JNIEXPORT jbyteArray JNICALL Java_app_reseam_sdk_Native_boltffi_1inspect_1apk_1json(JNIEnv *env, jclass cls, jbyteArray apk_path, jbyteArray split_paths_json) {
    bool _boltffi_input_error = false;

    jbyte _apk_path_stack[8];
    uintptr_t _apk_path_len = (uintptr_t)(*env)->GetArrayLength(env, apk_path);
    uint8_t* _apk_path_ptr = NULL;
    bool _apk_path_needs_release = false;
    if (_apk_path_len <= 8) {
        (*env)->GetByteArrayRegion(env, apk_path, 0, (jsize)_apk_path_len, _apk_path_stack);
        if (boltffi_exception_pending(env)) {
            _apk_path_len = 0;
            _boltffi_input_error = true;
        } else {
            _apk_path_ptr = (uint8_t*)_apk_path_stack;
        }
    } else {
        _apk_path_ptr = (uint8_t*)(*env)->GetByteArrayElements(env, apk_path, NULL);
        if (_apk_path_ptr == NULL) {
            _apk_path_len = 0;
            _boltffi_input_error = true;
        } else {
            _apk_path_needs_release = true;
        }
    }

    jbyte _split_paths_json_stack[8];
    uintptr_t _split_paths_json_len = (uintptr_t)(*env)->GetArrayLength(env, split_paths_json);
    uint8_t* _split_paths_json_ptr = NULL;
    bool _split_paths_json_needs_release = false;
    if (_split_paths_json_len <= 8) {
        (*env)->GetByteArrayRegion(env, split_paths_json, 0, (jsize)_split_paths_json_len, _split_paths_json_stack);
        if (boltffi_exception_pending(env)) {
            _split_paths_json_len = 0;
            _boltffi_input_error = true;
        } else {
            _split_paths_json_ptr = (uint8_t*)_split_paths_json_stack;
        }
    } else {
        _split_paths_json_ptr = (uint8_t*)(*env)->GetByteArrayElements(env, split_paths_json, NULL);
        if (_split_paths_json_ptr == NULL) {
            _split_paths_json_len = 0;
            _boltffi_input_error = true;
        } else {
            _split_paths_json_needs_release = true;
        }
    }
    FfiBuf_u8 _buf = {0};
    if (_boltffi_input_error) goto boltffi_input_cleanup;
    _buf = boltffi_inspect_apk_json((const uint8_t*)_apk_path_ptr, (uintptr_t)_apk_path_len, (const uint8_t*)_split_paths_json_ptr, (uintptr_t)_split_paths_json_len);
boltffi_input_cleanup:
    if (_apk_path_needs_release) (*env)->ReleaseByteArrayElements(env, apk_path, (jbyte*)_apk_path_ptr, JNI_ABORT);
    if (_split_paths_json_needs_release) (*env)->ReleaseByteArrayElements(env, split_paths_json, (jbyte*)_split_paths_json_ptr, JNI_ABORT);
    if (_boltffi_input_error) return NULL;
    return boltffi_buf_to_jbytearray(env, _buf);
}
JNIEXPORT jbyteArray JNICALL Java_app_reseam_sdk_Native_boltffi_1inspect_1json(JNIEnv *env, jclass cls, jbyteArray request_json) {
    bool _boltffi_input_error = false;

    jbyte _request_json_stack[8];
    uintptr_t _request_json_len = (uintptr_t)(*env)->GetArrayLength(env, request_json);
    uint8_t* _request_json_ptr = NULL;
    bool _request_json_needs_release = false;
    if (_request_json_len <= 8) {
        (*env)->GetByteArrayRegion(env, request_json, 0, (jsize)_request_json_len, _request_json_stack);
        if (boltffi_exception_pending(env)) {
            _request_json_len = 0;
            _boltffi_input_error = true;
        } else {
            _request_json_ptr = (uint8_t*)_request_json_stack;
        }
    } else {
        _request_json_ptr = (uint8_t*)(*env)->GetByteArrayElements(env, request_json, NULL);
        if (_request_json_ptr == NULL) {
            _request_json_len = 0;
            _boltffi_input_error = true;
        } else {
            _request_json_needs_release = true;
        }
    }
    FfiBuf_u8 _buf = {0};
    if (_boltffi_input_error) goto boltffi_input_cleanup;
    _buf = boltffi_inspect_json((const uint8_t*)_request_json_ptr, (uintptr_t)_request_json_len);
boltffi_input_cleanup:
    if (_request_json_needs_release) (*env)->ReleaseByteArrayElements(env, request_json, (jbyte*)_request_json_ptr, JNI_ABORT);
    if (_boltffi_input_error) return NULL;
    return boltffi_buf_to_jbytearray(env, _buf);
}
JNIEXPORT jbyteArray JNICALL Java_app_reseam_sdk_Native_boltffi_1patch_1json(JNIEnv *env, jclass cls, jbyteArray request_json, jlong event_sink) {
    bool _boltffi_input_error = false;

    jbyte _request_json_stack[8];
    uintptr_t _request_json_len = (uintptr_t)(*env)->GetArrayLength(env, request_json);
    uint8_t* _request_json_ptr = NULL;
    bool _request_json_needs_release = false;
    if (_request_json_len <= 8) {
        (*env)->GetByteArrayRegion(env, request_json, 0, (jsize)_request_json_len, _request_json_stack);
        if (boltffi_exception_pending(env)) {
            _request_json_len = 0;
            _boltffi_input_error = true;
        } else {
            _request_json_ptr = (uint8_t*)_request_json_stack;
        }
    } else {
        _request_json_ptr = (uint8_t*)(*env)->GetByteArrayElements(env, request_json, NULL);
        if (_request_json_ptr == NULL) {
            _request_json_len = 0;
            _boltffi_input_error = true;
        } else {
            _request_json_needs_release = true;
        }
    }
    FfiBuf_u8 _buf = {0};
    if (_boltffi_input_error) goto boltffi_input_cleanup;
    _buf = boltffi_patch_json((const uint8_t*)_request_json_ptr, (uintptr_t)_request_json_len, boltffi_create_patch_event_sink_handle((uint64_t)event_sink));
boltffi_input_cleanup:
    if (_request_json_needs_release) (*env)->ReleaseByteArrayElements(env, request_json, (jbyte*)_request_json_ptr, JNI_ABORT);
    if (_boltffi_input_error) return NULL;
    return boltffi_buf_to_jbytearray(env, _buf);
}

JNIEXPORT jbyteArray JNICALL Java_app_reseam_sdk_Native_boltffi_1last_1error_1message(JNIEnv *env, jclass cls) {
    FfiString out = { 0 };
    FfiStatus status = boltffi_last_error_message(&out);
    if (status.code != 0 || out.ptr == NULL || out.len == 0) {
        boltffi_free_string(out);
        return (*env)->NewByteArray(env, 0);
    }
    jbyteArray result = (*env)->NewByteArray(env, (jsize)out.len);
    if (result != NULL) {
        (*env)->SetByteArrayRegion(env, result, 0, (jsize)out.len, (const jbyte*)out.ptr);
    }
    boltffi_free_string(out);
    return result;
}

static jclass g_PatchEventSink_callbacks_class = NULL;
static jmethodID g_PatchEventSink_free_method = NULL;
static jmethodID g_PatchEventSink_clone_method = NULL;
static jmethodID g_PatchEventSink_on_event_method = NULL;

JNIEXPORT void JNICALL Java_app_reseam_sdk_Native_boltffiCallbackPatchEventSinkOnEvent(JNIEnv *env, jclass cls, jlong handle, jobject event_json) {
    if (handle == 0) {
        return;
    }
    bool _boltffi_input_error = false;
    jlong _event_json_size = (*env)->GetDirectBufferCapacity(env, event_json);
    uint8_t* _event_json_ptr = (uint8_t*)(*env)->GetDirectBufferAddress(env, event_json);
    uintptr_t _event_json_len = (_event_json_ptr && _event_json_size > 0) ? (uintptr_t)_event_json_size : 0;
    BoltFFICallbackHandle* callback = boltffi_jvm_callback_handle_ref(handle);
    if (_boltffi_input_error) goto boltffi_input_cleanup;
    if (callback == NULL || callback->handle == 0 || callback->vtable == NULL) {
        boltffi_throw_runtime(env, "invalid callback handle");
        goto boltffi_input_cleanup;
    }
    const PatchEventSinkVTable* vtable = (const PatchEventSinkVTable*)callback->vtable;
    if (vtable == NULL || vtable->on_event == NULL) {
        boltffi_throw_runtime(env, "callback method unavailable");
        goto boltffi_input_cleanup;
    }
    FfiStatus _status = {0};
    vtable->on_event(callback->handle, (const uint8_t*)_event_json_ptr, (uintptr_t)_event_json_len, &_status);
    if (_status.code != 0) {
        boltffi_throw_runtime(env, "callback invocation failed");
        goto boltffi_input_cleanup;
    }
boltffi_input_cleanup:
    if (boltffi_exception_pending(env) || _boltffi_input_error) return;
}

static void PatchEventSink_vtable_free(uint64_t handle) {
    JNIEnv* env;
    int attached = 0;
    jint get_env_result = (*g_jvm)->GetEnv(g_jvm, (void**)&env, JNI_VERSION_1_6);
    if (get_env_result == JNI_EDETACHED) {
        if (boltffi_attach_current_thread(g_jvm, &env) != JNI_OK) return;
        attached = 1;
    } else if (get_env_result != JNI_OK) return;
    (*env)->CallStaticVoidMethod(env, g_PatchEventSink_callbacks_class, g_PatchEventSink_free_method, (jlong)handle);
    boltffi_consume_pending_exception(env);
    if (attached) (*g_jvm)->DetachCurrentThread(g_jvm);
}

static uint64_t PatchEventSink_vtable_clone(uint64_t handle) {
    JNIEnv* env;
    int attached = 0;
    jint get_env_result = (*g_jvm)->GetEnv(g_jvm, (void**)&env, JNI_VERSION_1_6);
    if (get_env_result == JNI_EDETACHED) {
        if (boltffi_attach_current_thread(g_jvm, &env) != JNI_OK) return 0;
        attached = 1;
    } else if (get_env_result != JNI_OK) return 0;
    jlong result = (*env)->CallStaticLongMethod(env, g_PatchEventSink_callbacks_class, g_PatchEventSink_clone_method, (jlong)handle);
    if (boltffi_consume_pending_exception(env)) {
        if (attached) (*g_jvm)->DetachCurrentThread(g_jvm);
        return 0;
    }
    if (attached) (*g_jvm)->DetachCurrentThread(g_jvm);
    return (uint64_t)result;
}

JNIEXPORT jlong JNICALL Java_app_reseam_sdk_Native_boltffiCallbackPatchEventSinkClone(JNIEnv* env, jclass cls, jlong handle) {
    (void)cls;
    return boltffi_jvm_callback_handle_clone(env, boltffi_jvm_callback_handle_ref(handle));
}

JNIEXPORT void JNICALL Java_app_reseam_sdk_Native_boltffiCallbackPatchEventSinkRelease(JNIEnv* env, jclass cls, jlong handle) {
    (void)env; (void)cls;
    boltffi_jvm_callback_handle_release(boltffi_jvm_callback_handle_ref(handle));
}

static void PatchEventSink_vtable_on_event(uint64_t handle, const uint8_t* event_json_ptr, uintptr_t event_json_len, FfiStatus* _out_status) {
    JNIEnv* env;
    int attached = 0;
    jint get_env_result = (*g_jvm)->GetEnv(g_jvm, (void**)&env, JNI_VERSION_1_6);
    if (get_env_result == JNI_EDETACHED) {
        if (boltffi_attach_current_thread(g_jvm, &env) != JNI_OK) {
            _out_status->code = 1;
            return;
        }
        attached = 1;
    } else if (get_env_result != JNI_OK) {
        _out_status->code = 1;
        return;
    }
    jobject buf_event_json = NULL;
    if (event_json_ptr != NULL) {
        buf_event_json = (*env)->NewDirectByteBuffer(env, (void*)event_json_ptr, (jlong)event_json_len);
        if (buf_event_json == NULL) { boltffi_consume_pending_exception(env); }
    }
    (*env)->CallStaticVoidMethod(env, g_PatchEventSink_callbacks_class, g_PatchEventSink_on_event_method, (jlong)handle, buf_event_json);
    if (boltffi_consume_pending_exception(env)) {
        _out_status->code = 1;
        goto cleanup;
    }
    _out_status->code = 0;
cleanup:
    if (buf_event_json != NULL) (*env)->DeleteLocalRef(env, buf_event_json);
    if (attached) (*g_jvm)->DetachCurrentThread(g_jvm);
}

static PatchEventSinkVTable g_PatchEventSink_vtable = {
    .free = PatchEventSink_vtable_free,
    .clone = PatchEventSink_vtable_clone,
    .on_event = PatchEventSink_vtable_on_event,
};

static BoltFFICallbackInitResult init_PatchEventSink_callbacks(JNIEnv* env) {
    BoltFFIGlobalClassResult class_result =
        boltffi_lookup_global_class(env, "app/reseam/sdk/PatchEventSinkCallbacks", &g_PatchEventSink_callbacks_class);
    if (class_result == BOLTFFI_GLOBAL_CLASS_MISSING) {
        return BOLTFFI_CALLBACK_INIT_SKIPPED;
    }
    if (class_result != BOLTFFI_GLOBAL_CLASS_OK) {
        return BOLTFFI_CALLBACK_INIT_FATAL;
    }
    if (!boltffi_lookup_static_method(env, g_PatchEventSink_callbacks_class, "free", "(J)V", &g_PatchEventSink_free_method)) goto fail;
    if (!boltffi_lookup_static_method(env, g_PatchEventSink_callbacks_class, "clone", "(J)J", &g_PatchEventSink_clone_method)) goto fail;
    if (!boltffi_lookup_static_method(env, g_PatchEventSink_callbacks_class, "on_event", "(JLjava/nio/ByteBuffer;)V", &g_PatchEventSink_on_event_method)) goto fail;
    boltffi_register_patch_event_sink_vtable(&g_PatchEventSink_vtable);
    return BOLTFFI_CALLBACK_INIT_OK;
fail:
    (*env)->DeleteGlobalRef(env, g_PatchEventSink_callbacks_class);
    g_PatchEventSink_callbacks_class = NULL;
    return BOLTFFI_CALLBACK_INIT_FATAL;
}
JNIEXPORT void JNICALL JNI_OnUnload(JavaVM* vm, void* reserved) {
    JNIEnv* env;
    if ((*vm)->GetEnv(vm, (void**)&env, JNI_VERSION_1_6) != JNI_OK) {
        return;
    }
    if (g_callback_class != NULL) {
        (*env)->DeleteGlobalRef(env, g_callback_class);
        g_callback_class = NULL;
    }
    g_callback_method = NULL;
    if (g_PatchEventSink_callbacks_class != NULL) {
        (*env)->DeleteGlobalRef(env, g_PatchEventSink_callbacks_class);
        g_PatchEventSink_callbacks_class = NULL;
    }
    g_PatchEventSink_free_method = NULL;
    g_PatchEventSink_clone_method = NULL;
    g_PatchEventSink_on_event_method = NULL;
}