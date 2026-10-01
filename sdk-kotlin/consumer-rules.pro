# Patch bundles resolve these classes and members dynamically through JNI and their parent loader.
-keep class app.reseam.patch.** { *; }
-keep class kotlin.** { *; }
-keepattributes *Annotation*,Signature,InnerClasses,EnclosingMethod
