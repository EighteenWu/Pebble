# Sideload / JNI / reflection. Keep the app package and Tauri plugin
# classes so R8 cannot turn a missing method into a HyperOS silent kill.
-keep class com.qingj01.pebble.** { *; }
-keepclassmembers class com.qingj01.pebble.** { *; }
-keep class app.tauri.** { *; }
-keepclassmembers class app.tauri.** { *; }

# Add project specific ProGuard rules here.
# You can control the set of applied configuration files using the
# proguardFiles setting in build.gradle.
#
# For more details, see
#   http://developer.android.com/guide/developing/tools/proguard.html

# If your project uses WebView with JS, uncomment the following
# and specify the fully qualified class name to the JavaScript interface
# class:
#-keepclassmembers class fqcn.of.javascript.interface.for.webview {
#   public *;
#}

# Uncomment this to preserve the line number information for
# debugging stack traces.
#-keepattributes SourceFile,LineNumberTable

# If you keep the line number information, uncomment this to
# hide the original source file name.
#-renamesourcefileattribute SourceFile