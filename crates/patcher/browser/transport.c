/* SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
 * SPDX-License-Identifier: AGPL-3.0-or-later */
#include "jni.h"
#include <stdarg.h>
#include <stdlib.h>
#include <string.h>

typedef struct { uint32_t length; uint32_t width; unsigned char bytes[]; } Buffer;
static Buffer *failure;
static uint32_t failure_kind;

__attribute__((export_name("reseam_buffer_alloc")))
Buffer *reseam_buffer_alloc(uint32_t length, uint32_t width) {
    if (width == 0 || length > (SIZE_MAX - sizeof(Buffer)) / width) return NULL;
    Buffer *value = malloc(sizeof(Buffer) + (size_t)length * width);
    if (value != NULL) { value->length = length; value->width = width; }
    return value;
}

__attribute__((export_name("reseam_buffer_free")))
void reseam_buffer_free(Buffer *value) { free(value); }

__attribute__((export_name("reseam_bridge_reset")))
void reseam_bridge_reset(void) {
    free(failure);
    failure = NULL;
    failure_kind = 0;
}

__attribute__((export_name("reseam_bridge_error")))
Buffer *reseam_bridge_error(void) { return failure; }

__attribute__((export_name("reseam_bridge_error_kind")))
uint32_t reseam_bridge_error_kind(void) { return failure_kind; }

static jint throw_new(JNIEnv *env, jclass type, const char *message) {
    (void)env; (void)type;
    if (failure != NULL) return 0;
    size_t len = strlen(message);
    failure = reseam_buffer_alloc((uint32_t)len, 1);
    if (failure != NULL) memcpy(failure->bytes, message, len);
    failure_kind = 1;
    return 0;
}

static jlong browser_long(jlongArray array) {
    Buffer *buffer = array;
    jlong value = 0;
    if (buffer == NULL || buffer->length != 1 || buffer->width != 8) {
        throw_new(NULL, NULL, "invalid browser long carrier");
        return 0;
    }
    memcpy(&value, buffer->bytes, sizeof(value));
    return value;
}
static jlongArray browser_pack_long(jlong value) {
    if (failure_kind != 0) return NULL;
    Buffer *buffer = reseam_buffer_alloc(1, 8);
    if (buffer == NULL) { throw_new(NULL, NULL, "browser bridge allocation failed"); return NULL; }
    memcpy(buffer->bytes, &value, sizeof(value));
    return buffer;
}

static jclass find_class(JNIEnv *env, const char *name) { (void)env; return (jclass)name; }
static jboolean exception_check(JNIEnv *env) { (void)env; return failure_kind != 0; }
static void delete_local(JNIEnv *env, jobject value) { (void)env; (void)value; }
static jmethodID method_id(JNIEnv *env, jclass type, const char *name, const char *signature) {
    (void)env; (void)type; (void)signature; return (jmethodID)name;
}
static jobject new_object(JNIEnv *env, jclass type, jmethodID method, ...) {
    (void)env; (void)type; (void)method;
    va_list arguments;
    va_start(arguments, method);
    Buffer *value = va_arg(arguments, Buffer *);
    va_end(arguments);
    return value;
}
static jint throw_buffer(JNIEnv *env, jthrowable value) {
    (void)env;
    Buffer *input = value;
    if (failure_kind != 0) return 0;
    failure = reseam_buffer_alloc(input->length, input->width);
    if (failure != NULL) memcpy(failure->bytes, input->bytes, (size_t)input->length * input->width);
    free(input);
    failure_kind = 2;
    return 0;
}
static void *buffer_address(JNIEnv *env, jobject value) {
    (void)env; return value == NULL ? NULL : ((Buffer *)value)->bytes;
}
static jlong buffer_capacity(JNIEnv *env, jobject value) {
    (void)env; return value == NULL ? -1 : (jlong)((Buffer *)value)->length * ((Buffer *)value)->width;
}
static jsize array_length(JNIEnv *env, jarray value) {
    (void)env; return value == NULL ? 0 : (jsize)((Buffer *)value)->length;
}
static jbyteArray new_bytes(JNIEnv *env, jsize count) {
    if (count < 0) { throw_new(env, NULL, "negative byte array length"); return NULL; }
    Buffer *value = reseam_buffer_alloc((uint32_t)count, 1);
    if (value == NULL) throw_new(env, NULL, "browser bridge allocation failed");
    return value;
}
static void region(JNIEnv *env, Buffer *array, jsize start, jsize len, void *target, size_t width, int write) {
    if (array == NULL || start < 0 || len < 0 || (uint32_t)start > array->length ||
        (uint32_t)len > array->length - (uint32_t)start || array->width != width) {
        throw_new(env, NULL, "invalid browser bridge array range"); return;
    }
    void *source = array->bytes + (size_t)start * width;
    if (write) memcpy(source, target, (size_t)len * width);
    else memcpy(target, source, (size_t)len * width);
}
static void set_bytes(JNIEnv *env, jbyteArray a, jsize s, jsize n, const jbyte *data) { region(env, a, s, n, (void *)data, 1, 1); }
static void get_bytes(JNIEnv *env, jbyteArray a, jsize s, jsize n, jbyte *data) { region(env, a, s, n, data, 1, 0); }
static void get_shorts(JNIEnv *env, jshortArray a, jsize s, jsize n, jshort *data) { region(env, a, s, n, data, 2, 0); }
static void get_ints(JNIEnv *env, jintArray a, jsize s, jsize n, jint *data) { region(env, a, s, n, data, 4, 0); }
static jbyte *byte_elements(JNIEnv *env, jbyteArray a, jboolean *copy) { if (copy) *copy = 0; return buffer_address(env, a); }
static jshort *short_elements(JNIEnv *env, jshortArray a, jboolean *copy) { if (copy) *copy = 0; return buffer_address(env, a); }
static jint *int_elements(JNIEnv *env, jintArray a, jboolean *copy) { if (copy) *copy = 0; return buffer_address(env, a); }
static void release_bytes(JNIEnv *env, jbyteArray a, jbyte *data, jint mode) { (void)env; (void)a; (void)data; (void)mode; }
static void release_shorts(JNIEnv *env, jshortArray a, jshort *data, jint mode) { (void)env; (void)a; (void)data; (void)mode; }
static void release_ints(JNIEnv *env, jintArray a, jint *data, jint mode) { (void)env; (void)a; (void)data; (void)mode; }
static jint register_natives(JNIEnv *env, jclass type, const JNINativeMethod *methods, jint count) { (void)env; (void)type; (void)methods; (void)count; return 0; }
static const struct BrowserTransport browser_transport = {
    find_class, throw_new, throw_buffer, exception_check, delete_local, method_id, new_object,
    buffer_address, buffer_capacity, array_length, new_bytes, set_bytes,
    get_bytes, get_shorts, get_ints, byte_elements, short_elements, int_elements,
    release_bytes, release_shorts, release_ints, register_natives
};
static JNIEnv browser_env = &browser_transport;
