// SPDX-FileCopyrightText: 2026 Reseam contributors <https://reseam.app>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What the overview recognises by name: Kotlin's runtime, libraries, SDKs
//! and protection markers. Entries come from real APKs; add one when an APK
//! shows it.

use reseam_model::analysis::SdkCategory;

/// Types of Kotlin's runtime library that compiled Kotlin refers to.
pub(super) const KOTLIN_TYPES: [&str; 3] = [
    "Lkotlin/Metadata;",
    "Lkotlin/jvm/internal/Intrinsics;",
    "Lkotlin/Unit;",
];

/// Packages of libraries apps bundle, in Java form; the longest match wins.
pub(super) const LIBRARIES: &[&str] = &[
    "android.support",
    "androidx",
    "coil",
    "com.adjust.sdk",
    "com.airbnb.lottie",
    "com.android.billingclient",
    "com.applovin",
    "com.appsflyer",
    "com.bumptech.glide",
    "com.facebook",
    "com.facebook.react",
    "com.google.android.datatransport",
    "com.google.android.exoplayer2",
    "com.google.android.gms",
    "com.google.android.material",
    "com.google.android.play",
    "com.google.common",
    "com.google.firebase",
    "com.google.gson",
    "com.google.mlkit",
    "com.google.protobuf",
    "com.revenuecat.purchases",
    "com.squareup",
    "com.unity3d",
    "dagger",
    "io.flutter",
    "io.grpc",
    "io.ktor",
    "io.reactivex",
    "io.sentry",
    "j$",
    "javax",
    "kotlin",
    "kotlinx",
    "okhttp3",
    "okio",
    "org.chromium",
    "org.intellij",
    "org.jetbrains",
    "retrofit2",
    "timber",
];

/// An SDK and what shows it: its package, and manifest permissions or
/// meta-data names it requires.
pub(super) struct Sdk {
    pub(super) category: SdkCategory,
    pub(super) name: &'static str,
    pub(super) package: &'static str,
    pub(super) manifest: &'static [&'static str],
}

const fn sdk(
    category: SdkCategory,
    name: &'static str,
    package: &'static str,
    manifest: &'static [&'static str],
) -> Sdk {
    Sdk {
        category,
        name,
        package,
        manifest,
    }
}

pub(super) const SDKS: &[Sdk] = &[
    sdk(
        SdkCategory::Billing,
        "Play Billing",
        "com.android.billingclient",
        &["com.android.vending.BILLING"],
    ),
    sdk(
        SdkCategory::Billing,
        "RevenueCat",
        "com.revenuecat.purchases",
        &[],
    ),
    sdk(SdkCategory::Billing, "Adapty", "com.adapty", &[]),
    sdk(SdkCategory::Billing, "Qonversion", "com.qonversion", &[]),
    sdk(
        SdkCategory::Ads,
        "AdMob",
        "com.google.android.gms.ads",
        &["com.google.android.gms.ads.APPLICATION_ID"],
    ),
    sdk(
        SdkCategory::Ads,
        "AppLovin",
        "com.applovin",
        &["applovin.sdk.key"],
    ),
    sdk(SdkCategory::Ads, "Unity Ads", "com.unity3d.ads", &[]),
    sdk(SdkCategory::Ads, "ironSource", "com.ironsource", &[]),
    sdk(
        SdkCategory::Ads,
        "Meta Audience Network",
        "com.facebook.ads",
        &[],
    ),
    sdk(SdkCategory::Ads, "Liftoff Vungle", "com.vungle", &[]),
    sdk(SdkCategory::Ads, "Mintegral", "com.mbridge", &[]),
    sdk(
        SdkCategory::Ads,
        "Pangle",
        "com.bytedance.sdk.openadsdk",
        &[],
    ),
    sdk(SdkCategory::Ads, "InMobi", "com.inmobi", &[]),
    sdk(
        SdkCategory::SignIn,
        "Google Sign-In",
        "com.google.android.gms.auth.api.signin",
        &[],
    ),
    sdk(
        SdkCategory::SignIn,
        "Credential Manager",
        "androidx.credentials",
        &[],
    ),
    sdk(
        SdkCategory::SignIn,
        "Sign in with Google",
        "com.google.android.libraries.identity.googleid",
        &[],
    ),
    sdk(
        SdkCategory::SignIn,
        "Firebase Auth",
        "com.google.firebase.auth",
        &[],
    ),
    sdk(
        SdkCategory::SignIn,
        "Facebook Login",
        "com.facebook.login",
        &["com.facebook.sdk.ApplicationId"],
    ),
    sdk(
        SdkCategory::Analytics,
        "Firebase Analytics",
        "com.google.firebase.analytics",
        &[],
    ),
    sdk(SdkCategory::Analytics, "Amplitude", "com.amplitude", &[]),
    sdk(SdkCategory::Analytics, "Mixpanel", "com.mixpanel", &[]),
    sdk(SdkCategory::Analytics, "Segment", "com.segment", &[]),
    sdk(SdkCategory::Analytics, "AppsFlyer", "com.appsflyer", &[]),
    sdk(SdkCategory::Analytics, "Adjust", "com.adjust.sdk", &[]),
    sdk(SdkCategory::Analytics, "Branch", "io.branch", &[]),
    sdk(SdkCategory::Analytics, "Braze", "com.braze", &[]),
    sdk(SdkCategory::Analytics, "AppMetrica", "io.appmetrica", &[]),
    sdk(
        SdkCategory::CrashReporting,
        "Crashlytics",
        "com.google.firebase.crashlytics",
        &[],
    ),
    sdk(
        SdkCategory::CrashReporting,
        "Sentry",
        "io.sentry",
        &["io.sentry.dsn"],
    ),
    sdk(
        SdkCategory::CrashReporting,
        "Bugsnag",
        "com.bugsnag",
        &["com.bugsnag.android.API_KEY"],
    ),
    sdk(SdkCategory::CrashReporting, "ACRA", "org.acra", &[]),
    sdk(SdkCategory::CrashReporting, "Instabug", "com.instabug", &[]),
    sdk(SdkCategory::CrashReporting, "Datadog", "com.datadog", &[]),
];

/// A protection, and the class packages and native libraries that show it.
pub(super) struct Protection {
    pub(super) name: &'static str,
    pub(super) packages: &'static [&'static str],
    pub(super) libraries: &'static [&'static str],
}

pub(super) const PROTECTIONS: &[Protection] = &[Protection {
    name: "PairIP",
    packages: &["com.pairip"],
    libraries: &["libpairipcore.so"],
}];
