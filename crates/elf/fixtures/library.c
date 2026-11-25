// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

extern int lookup(const char *key);
extern void subscribe(int (*callback)(void));

// On 32-bit ARM, level is ARM code and the rest Thumb, as in libraries that mix both.
#if defined(__arm__)
#define ARM_CODE __attribute__((target("arm")))
#else
#define ARM_CODE
#endif

__attribute__((noinline)) static int has_access(void) {
    return lookup("2026-11-access-check") == 2;
}

__attribute__((noinline)) ARM_CODE static int level(void) {
    return lookup("feature-level") + lookup("feature-level-bonus");
}

__attribute__((noinline)) static int shared_a(void) {
    return lookup("shared-key") + 1;
}

__attribute__((noinline)) static int shared_b(void) {
    return lookup("shared-key") + lookup("only-in-b");
}

static int pointed(void) {
    return lookup("reached through a pointer");
}

static int subscribed(void) {
    return lookup("reached through a callback");
}

int (*const callbacks[])(void) = {pointed};

int exported_check(void) {
    subscribe(subscribed);
    return has_access() + shared_a() + level() + shared_b();
}
