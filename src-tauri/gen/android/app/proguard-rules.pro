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

# ---------------------------------------------------------------------------
# Miembros que Rust llama por JNI *por su nombre*.
#
# R8 renombra los métodos en release, y desde Rust se invocan con
# `env.call_method(activity, "publishToMediaStore", …)`. Si el minificador los
# renombra, la llamada falla en tiempo de ejecución con `NoSuchMethodError` y
# —como el error se ignora para no romper la descarga— el síntoma sería que el
# vídeo nunca aparece en la galería, sin ningún mensaje. De ahí que haya que
# conservarlos explícitamente.
# ---------------------------------------------------------------------------
-keepclassmembers class com.ramses.downlink.MainActivity {
  public java.lang.String publishToMediaStore(java.lang.String, java.lang.String);
  public java.lang.String openDownloaded(java.lang.String);
  public void pickWallpaper();
  public void pickCookiesFile();
  public void startDownloadService(java.lang.String);
  public void updateDownloadService(java.lang.String, int);
  public void stopDownloadService();
}

# `MediaStoreBridge` no es llamado por JNI, pero sus métodos forman la API que
# usa `MainActivity`; se conserva la clase entera por claridad y para que un
# cambio futuro (llamarla también desde Rust) no dependa de este detalle.
-keep class com.ramses.downlink.MediaStoreBridge {
  public *;
}

# El servicio en primer plano lo arranca Android por su nombre desde el
# manifiesto; R8 no debe tocarlo ni renombrarlo.
-keep class com.ramses.downlink.DownloadService { *; }
