/* SPDX-FileCopyrightText: 2026 Cossale <hello@auna.li>
 * SPDX-License-Identifier: AGPL-3.0-or-later */
#pragma once
#include <stdint.h>
#include <stddef.h>

/* This is the transport contract used to compile BoltFFI's generated glue,
 * not a JVM JNI implementation. Objects are worker-owned buffer descriptors. */
typedef uint8_t jboolean;
typedef int8_t jbyte;
typedef uint16_t jchar;
typedef int16_t jshort;
typedef int32_t jint;
typedef int64_t jlong;
typedef float jfloat;
typedef double jdouble;
typedef jint jsize;
typedef void *jobject;
typedef jobject jclass;
typedef jobject jthrowable;
typedef jobject jmethodID;
typedef jobject jarray;
typedef jarray jbyteArray;
typedef jarray jshortArray;
typedef jarray jintArray;
typedef jarray jlongArray;
typedef struct { const char *name; const char *signature; void *fnPtr; } JNINativeMethod;
typedef const struct BrowserTransport *JNIEnv;
#define JNIEXPORT
#define JNICALL
#define JNI_TRUE 1
#define JNI_FALSE 0
#define JNI_ABORT 2

struct BrowserTransport {
    jclass (*FindClass)(JNIEnv *, const char *);
    jint (*ThrowNew)(JNIEnv *, jclass, const char *);
    jint (*Throw)(JNIEnv *, jthrowable);
    jboolean (*ExceptionCheck)(JNIEnv *);
    void (*DeleteLocalRef)(JNIEnv *, jobject);
    jmethodID (*GetMethodID)(JNIEnv *, jclass, const char *, const char *);
    jobject (*NewObject)(JNIEnv *, jclass, jmethodID, ...);
    void *(*GetDirectBufferAddress)(JNIEnv *, jobject);
    jlong (*GetDirectBufferCapacity)(JNIEnv *, jobject);
    jsize (*GetArrayLength)(JNIEnv *, jarray);
    jbyteArray (*NewByteArray)(JNIEnv *, jsize);
    void (*SetByteArrayRegion)(JNIEnv *, jbyteArray, jsize, jsize, const jbyte *);
    void (*GetByteArrayRegion)(JNIEnv *, jbyteArray, jsize, jsize, jbyte *);
    void (*GetShortArrayRegion)(JNIEnv *, jshortArray, jsize, jsize, jshort *);
    void (*GetIntArrayRegion)(JNIEnv *, jintArray, jsize, jsize, jint *);
    jbyte *(*GetByteArrayElements)(JNIEnv *, jbyteArray, jboolean *);
    jshort *(*GetShortArrayElements)(JNIEnv *, jshortArray, jboolean *);
    jint *(*GetIntArrayElements)(JNIEnv *, jintArray, jboolean *);
    void (*ReleaseByteArrayElements)(JNIEnv *, jbyteArray, jbyte *, jint);
    void (*ReleaseShortArrayElements)(JNIEnv *, jshortArray, jshort *, jint);
    void (*ReleaseIntArrayElements)(JNIEnv *, jintArray, jint *, jint);
    jint (*RegisterNatives)(JNIEnv *, jclass, const JNINativeMethod *, jint);
};
