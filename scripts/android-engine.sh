#!/usr/bin/env bash
#
# Motores nativos de Android (arm64) para Downlink.
#
# Construye y empaqueta lo que el APK necesita para descargar sin depender de
# nada instalado en el teléfono:
#
#   OpenSSL    → TLS para CPython y ffmpeg (estático)
#   ffmpeg     → unir audio y vídeo, cortar, extraer audio (estático, con
#                libmp3lame, libopus y libvorbis para poder recomprimir)
#   ffprobe    → leer duración, códecs y streams, que es lo que yt-dlp consulta
#                al incrustar metadatos o portada (sin él, el postprocesado falla)
#   bzip2/xz/zstd → compresión que el stdlib de Python da por hecha
#   CPython    → intérprete que lee el zipapp de yt-dlp (libpython estático)
#   quickjs    → motor JS que yt-dlp pide para YouTube
#   yt-dlp     → el *zipapp* oficial, para funcionar nada más instalar
#
# Cada binario acaba en jniLibs con nombre `lib*.so`, que es lo único que el
# empaquetador de Android extrae a `nativeLibraryDir` (el único sitio desde
# Android 10 que se puede *ejecutar*). El stdlib de Python viaja como un zip
# con el mismo truco y se descomprime la primera vez que arranca la app.
#
# Uso:
#   scripts/android-engine.sh [paso ...]     # sin pasos: todos
#
# Pasos: openssl lame opus vorbis bzip2 xz zstd ffmpeg cpython quickjs package
#
# Variables útiles:
#   ANDROID_ENGINE_WORK  carpeta de trabajo (por defecto, caché del usuario)
#   ANDROID_API          nivel de API mínimo (por defecto 24)
#   YTDLP_VERSION        versión de yt-dlp a incluir (por defecto, la última)
#
# Se puede reanudar: cada paso se salta si su resultado ya existe.
set -euo pipefail

NDK="${ANDROID_NDK_ROOT:-${NDK_HOME:-$HOME/Android/sdk/ndk/28.2.13676358}}"
export ANDROID_NDK_ROOT="$NDK"
export ANDROID_NDK_HOME="$NDK"
API="${ANDROID_API:-24}"
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
JNI_DIR="$REPO/src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a"

# Sitio de trabajo: fuentes, objetos y la instalación resultante. Es una caché,
# así que vive fuera del repo y se puede borrar sin miedo.
WORK="${ANDROID_ENGINE_WORK:-${XDG_CACHE_HOME:-$HOME/.cache}/downlink-engine}"
SRC="$WORK/src"
PREFIX="$WORK/prefix"
STAMPS="$WORK/stamps"
DEPS="$WORK/deps"

PY_VERSION="3.14.7"
PY_SHORT="3.14"
OPENSSL_VERSION="3.5.4"
LAME_VERSION="3.100"
OPUS_VERSION="1.5.2"
OGG_VERSION="1.3.5"
VORBIS_VERSION="1.3.7"
BZIP2_VERSION="1.0.8"
XZ_VERSION="5.6.4"
ZSTD_VERSION="1.5.7"
FFMPEG_VERSION="7.1.1"
QUICKJS_TAG="v0.17.0"

TOOLCHAIN="$NDK/toolchains/llvm/prebuilt/linux-x86_64"
BIN="$TOOLCHAIN/bin"
SYSROOT="$TOOLCHAIN/sysroot"
TRIPLE="aarch64-linux-android"
CLANG="$BIN/$TRIPLE$API-clang"
CLANGXX="$BIN/$TRIPLE$API-clang++"
export AR="$BIN/llvm-ar"
export RANLIB="$BIN/llvm-ranlib"
export NM="$BIN/llvm-nm"
export STRIP="$BIN/llvm-strip"
export LD="$BIN/ld.lld"

# OpenSSL es el único que necesita el `bin` del NDK en el PATH: su Makefile llama
# al compilador y a `llvm-ar` por su nombre pelado. Se hace dentro de su paso,
# en un subproceso, para que un `clang` genérico no se cuele en las demás
# compilaciones (autoconf del host lo confundiría con el suyo).

log() { printf '\n\033[1m==> %s\033[0m\n' "$*"; }

done_step() { [ -f "$STAMPS/$1" ]; }
mark() { mkdir -p "$STAMPS"; touch "$STAMPS/$1"; }

fetch() {
	local url="$1" file="$2"
	if [ ! -f "$SRC/$file" ]; then
		curl -fsSL --retry 3 -o "$SRC/$file" "$url"
		# Algunos hospedajes (SourceForge) responden con una página de "empieza
		# la descarga" en vez del fichero: mejor fallar aquí que al desempaquetar.
		if head -c 200 "$SRC/$file" | grep -qi '<!doctype html\|<html'; then
			rm -f "$SRC/$file"
			echo "La descarga de $file devolvió HTML, no el fichero." >&2
			exit 1
		fi
	fi
}

unpack() {
	local file="$1" dir="$2"
	if [ -d "$SRC/$dir" ] && [ -n "$(ls -A "$SRC/$dir")" ]; then
		return 0
	fi
	# Una carpeta vacía es de un intento anterior que falló a medias: se rehace.
	rm -rf "$SRC/$dir"
	mkdir -p "$SRC/$dir"
	case "$file" in
	*.tar.gz | *.tgz) tar -xzf "$SRC/$file" -C "$SRC/$dir" --strip-components=1 ;;
	*.tar.xz) tar -xJf "$SRC/$file" -C "$SRC/$dir" --strip-components=1 ;;
	*.zip) unzip -q "$SRC/$file" -d "$SRC/$dir" ;;
	*) echo "Formato no soportado: $file" >&2; exit 1 ;;
	esac
}

# --- OpenSSL --------------------------------------------------------------

step_openssl() (
	done_step openssl && return 0
	export PATH="$BIN:$PATH"
	log "OpenSSL $OPENSSL_VERSION → prefijo"
	fetch "https://www.openssl.org/source/openssl-$OPENSSL_VERSION.tar.gz" openssl.tar.gz
	unpack openssl.tar.gz "openssl-$OPENSSL_VERSION"
	cd "$SRC/openssl-$OPENSSL_VERSION"
	make clean >/dev/null 2>&1 || true
	./Configure "android-arm64" \
		-D__ANDROID_API__="$API" \
		--prefix="$PREFIX" --openssldir="$PREFIX/ssl" \
		no-shared no-tests no-ui-console -fPIC
	# Solo las librerías: las herramientas de línea de órdenes no hacen falta y
	# se compilan en C++, que aquí no está preparado.
	make -j"$(nproc)" build_libs
	make install_dev
	mark openssl
)

# --- Códecs de audio que ffmpeg necesita para recomprimir ----------------

step_lame() {
	done_step lame && return 0
	log "libmp3lame $LAME_VERSION"
	fetch "https://downloads.sourceforge.net/project/lame/lame/$LAME_VERSION/lame-$LAME_VERSION.tar.gz" lame.tar.gz
	unpack lame.tar.gz "lame-$LAME_VERSION"
	cd "$SRC/lame-$LAME_VERSION"
	CC="$CLANG" AR="$AR" RANLIB="$RANLIB" STRIP="$STRIP" \
		./configure --host="$TRIPLE" --prefix="$PREFIX" \
		--disable-shared --enable-static --disable-frontend
	make -j"$(nproc)"
	make install
	mark lame
}

step_opus() {
	done_step opus && return 0
	log "libopus $OPUS_VERSION"
	fetch "https://downloads.xiph.org/releases/opus/opus-$OPUS_VERSION.tar.gz" opus.tar.gz
	unpack opus.tar.gz "opus-$OPUS_VERSION"
	cd "$SRC/opus-$OPUS_VERSION"
	CC="$CLANG" AR="$AR" RANLIB="$RANLIB" STRIP="$STRIP" \
		./configure --host="$TRIPLE" --prefix="$PREFIX" \
		--disable-shared --enable-static --disable-doc --disable-extra-programs
	make -j"$(nproc)"
	make install
	# Se enlaza estático: las dependencias privadas tienen que estar en `Libs`,
	# no en `Libs.private`, o el `configure` de ffmpeg no las verá.
	sed -i 's|^Libs: \(.*\)|Libs: \1 -lm|' "$PREFIX/lib/pkgconfig/opus.pc"
	mark opus
}

step_vorbis() {
	done_step vorbis && return 0
	log "libogg $OGG_VERSION + libvorbis $VORBIS_VERSION"
	fetch "https://downloads.xiph.org/releases/ogg/libogg-$OGG_VERSION.tar.gz" libogg.tar.gz
	fetch "https://downloads.xiph.org/releases/vorbis/libvorbis-$VORBIS_VERSION.tar.gz" libvorbis.tar.gz
	unpack libogg.tar.gz "libogg-$OGG_VERSION"
	unpack libvorbis.tar.gz "libvorbis-$VORBIS_VERSION"
	cd "$SRC/libogg-$OGG_VERSION"
	CC="$CLANG" AR="$AR" RANLIB="$RANLIB" STRIP="$STRIP" \
		./configure --host="$TRIPLE" --prefix="$PREFIX" --disable-shared --enable-static
	make -j"$(nproc)"
	make install
	cd "$SRC/libvorbis-$VORBIS_VERSION"
	CC="$CLANG" AR="$AR" RANLIB="$RANLIB" STRIP="$STRIP" \
		PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig" \
		./configure --host="$TRIPLE" --prefix="$PREFIX" \
		--disable-shared --enable-static --disable-docs --disable-examples \
		--with-ogg="$PREFIX"
	make -j"$(nproc)"
	make install
	# Igual que con opus: enlace estático, las dependencias van en `Libs`.
	sed -i 's|^Libs: .*|Libs: -L${libdir} -lvorbis -lm -logg|' "$PREFIX/lib/pkgconfig/vorbis.pc"
	sed -i 's|^Libs: .*|Libs: -L${libdir} -lvorbisenc -lvorbis -logg -lm|' "$PREFIX/lib/pkgconfig/vorbisenc.pc"
	sed -i 's|^Libs: .*|Libs: -L${libdir} -lvorbisfile -lvorbis -logg -lm|' "$PREFIX/lib/pkgconfig/vorbisfile.pc"
	mark vorbis
}

# --- Compresión que el stdlib de Python pide (bz2, lzma, zstd) -------------
#
# CPython no los da por hechos: si `configure` no los ve, se queda sin esos
# módulos. Y como están escritos en el `Setup.stdlib` con dependencia directa,
# conviene traerlos para que el intérprete del APK sea un CPython completo.

step_bzip2() {
	done_step bzip2 && return 0
	log "bzip2 $BZIP2_VERSION"
	fetch "https://sourceware.org/pub/bzip2/bzip2-$BZIP2_VERSION.tar.gz" bzip2.tar.gz
	unpack bzip2.tar.gz "bzip2-$BZIP2_VERSION"
	cd "$SRC/bzip2-$BZIP2_VERSION"
	make clean >/dev/null 2>&1 || true
	# Solo la librería: los programas de línea de órdenes no viajan en el APK.
	make -j"$(nproc)" libbz2.a CC="$CLANG" AR="$AR" RANLIB="$RANLIB" CFLAGS="-O2 -fPIC"
	install -D libbz2.a "$PREFIX/lib/libbz2.a"
	install -D bzlib.h "$PREFIX/include/bzlib.h"
	# Un .pc a mano: `configure` de CPython busca así estas dependencias, y sin
	# él el sondeo miraría las librerías del sistema anfitrión.
	mkdir -p "$PREFIX/lib/pkgconfig"
	cat > "$PREFIX/lib/pkgconfig/bzip2.pc" <<-EOF
		prefix=$PREFIX
		Name: bzip2
		Description: bzip2
		Version: $BZIP2_VERSION
		Cflags: -I\${prefix}/include
		Libs: -L\${prefix}/lib -lbz2
	EOF
	mark bzip2
}

step_xz() {
	done_step xz && return 0
	log "xz/liblzma $XZ_VERSION"
	fetch "https://github.com/tukaani-project/xz/releases/download/v$XZ_VERSION/xz-$XZ_VERSION.tar.gz" xz.tar.gz
	unpack xz.tar.gz "xz-$XZ_VERSION"
	cd "$SRC/xz-$XZ_VERSION"
	[ -f Makefile ] && make distclean >/dev/null 2>&1 || true
	CC="$CLANG" AR="$AR" RANLIB="$RANLIB" STRIP="$STRIP" \
		./configure --host="$TRIPLE" --prefix="$PREFIX" \
		--disable-shared --enable-static --disable-nls --disable-doc \
		--disable-scripts --disable-xz --disable-xzdec --disable-lzmadec \
		--disable-lzmainfo --disable-ifunc
	make -j"$(nproc)"
	make install
	mark xz
}

step_zstd() {
	done_step zstd && return 0
	log "zstd $ZSTD_VERSION"
	fetch "https://github.com/facebook/zstd/releases/download/v$ZSTD_VERSION/zstd-$ZSTD_VERSION.tar.gz" zstd.tar.gz
	unpack zstd.tar.gz "zstd-$ZSTD_VERSION"
	cd "$SRC/zstd-$ZSTD_VERSION/lib"
	make clean >/dev/null 2>&1 || true
	make -j"$(nproc)" libzstd.a CC="$CLANG" AR="$AR" RANLIB="$RANLIB" CFLAGS="-O2 -fPIC"
	install -D libzstd.a "$PREFIX/lib/libzstd.a"
	install -D zstd.h "$PREFIX/include/zstd.h"
	install -D zstd_errors.h "$PREFIX/include/zstd_errors.h"
	install -D zdict.h "$PREFIX/include/zdict.h"
	mkdir -p "$PREFIX/lib/pkgconfig"
	cat > "$PREFIX/lib/pkgconfig/libzstd.pc" <<-EOF
		prefix=$PREFIX
		Name: libzstd
		Description: zstd
		Version: $ZSTD_VERSION
		Cflags: -I\${prefix}/include
		Libs: -L\${prefix}/lib -lzstd
	EOF
	mark zstd
}

# --- ffmpeg ---------------------------------------------------------------

step_ffmpeg() {
	done_step ffmpeg && return 0
	log "ffmpeg $FFMPEG_VERSION (estático)"
	fetch "https://ffmpeg.org/releases/ffmpeg-$FFMPEG_VERSION.tar.xz" ffmpeg.tar.xz
	unpack ffmpeg.tar.xz "ffmpeg-$FFMPEG_VERSION"
	cd "$SRC/ffmpeg-$FFMPEG_VERSION"
	PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig" PKG_CONFIG_LIBDIR="$PREFIX/lib/pkgconfig" \
		./configure \
		--prefix="$PREFIX" \
		--target-os=android --arch=aarch64 --cpu=armv8-a \
		--enable-cross-compile --cross-prefix="$BIN/llvm-" \
		--cc="$CLANG" --cxx="$CLANGXX" \
		--pkg-config=pkg-config \
		--sysroot="$SYSROOT" \
		--enable-static --disable-shared --enable-pic \
		--enable-openssl --enable-libmp3lame --enable-libopus --enable-libvorbis \
		--extra-cflags="-I$PREFIX/include" \
		--extra-ldflags="-L$PREFIX/lib" \
		--disable-doc --disable-debug \
		--disable-ffplay \
		--disable-avdevice --disable-postproc
	# `ffprobe` **sí** se compila: yt-dlp lo usa para leer duración, códecs y
	# streams antes de incrustar metadatos o portada, y sin él el postprocesado
	# falla con «Unable to embed using ffprobe & ffmpeg; ffprobe not found».
	# Solo se deja fuera `ffplay`, que es un reproductor con ventana y aquí no
	# tiene ningún sentido.
	# A propósito no se recortan decodificadores ni demuxers: un vídeo de un
	# sitio raro puede traer cualquier cosa y aquí no hay forma de probarlo.
	make -j"$(nproc)"
	make install
	mark ffmpeg
}

# --- CPython --------------------------------------------------------------

step_cpython() {
	done_step cpython && return 0
	log "CPython $PY_VERSION (libpython estático) → prefijo"
	fetch "https://www.python.org/ftp/python/$PY_VERSION/Python-$PY_VERSION.tgz" "Python-$PY_VERSION.tgz"
	unpack "Python-$PY_VERSION.tgz" "Python-$PY_VERSION"
	cd "$SRC/Python-$PY_VERSION"
	[ -f Makefile ] && make distclean >/dev/null 2>&1 || true
	CC="$CLANG" CXX="$CLANGXX" AR="$AR" RANLIB="$RANLIB" STRIP="$STRIP" \
		CPPFLAGS="-I$PREFIX/include" LDFLAGS="-L$PREFIX/lib" \
		PKG_CONFIG_LIBDIR="$PREFIX/lib/pkgconfig:$PREFIX/share/pkgconfig" \
		./configure \
		--host="$TRIPLE" --build="$(uname -m)-pc-linux-gnu" \
		--with-build-python="$(command -v python3)" \
		--prefix="$PREFIX" \
		--disable-shared \
		--without-ensurepip --disable-test-modules \
		--with-openssl="$PREFIX" --with-openssl-rpath=no \
		ac_cv_file__dev_ptmx=no ac_cv_file__dev_ptc=no
	make -j"$(nproc)"
	make install
	mark cpython
}

# --- quickjs --------------------------------------------------------------

step_quickjs() {
	done_step quickjs && return 0
	log "quickjs-ng $QUICKJS_TAG (amalgama)"
	fetch "https://github.com/quickjs-ng/quickjs/releases/download/$QUICKJS_TAG/quickjs-amalgam.zip" quickjs-amalgam.zip
	# La amalgama es solo la biblioteca: `main` (la herramienta `qjs`) vive en
	# `qjs.c`, que se toma del código del mismo tag.
	fetch "https://github.com/quickjs-ng/quickjs/archive/refs/tags/$QUICKJS_TAG.tar.gz" quickjs-src.tar.gz
	rm -rf "$SRC/quickjs" "$SRC/quickjs-src"
	unpack quickjs-amalgam.zip quickjs
	unpack quickjs-src.tar.gz quickjs-src
	cd "$SRC/quickjs"
	# `QJS_BUILD_LIBC` mete en la amalgama la parte de consola (js_std_*) que la
	# herramienta necesita; `repl.c` y `standalone.c` vienen ya generados en el
	# paquete del tag. `-ldl` es por la carga de módulos nativos.
	"$CLANG" -O2 -DNDEBUG -fPIC -D_GNU_SOURCE -DQJS_BUILD_LIBC -I. -o qjs \
		quickjs-amalgam.c "$SRC/quickjs-src/qjs.c" \
		"$SRC/quickjs-src/gen/repl.c" "$SRC/quickjs-src/gen/standalone.c" -lm -ldl
	"$STRIP" --strip-all qjs
	mark quickjs
}

# --- Empaquetado ----------------------------------------------------------

step_package() {
	log "Empaquetando los motores en $JNI_DIR"
	mkdir -p "$JNI_DIR" "$WORK/out"

	# El intérprete de Python: es un ejecutable, pero Android solo extrae a
	# nativeLibraryDir los ficheros con nombre `lib*.so`, así que se renombra.
	install -m 755 "$PREFIX/bin/python$PY_SHORT" "$WORK/out/libpython.so"
	"$STRIP" --strip-all "$WORK/out/libpython.so"

	# ffmpeg (unir audio y vídeo, recomprimir, extraer audio).
	install -m 755 "$PREFIX/bin/ffmpeg" "$WORK/out/libffmpeg.so"
	"$STRIP" --strip-all "$WORK/out/libffmpeg.so"

	# ffprobe lee lo que yt-dlp necesita saber antes de reescribir un fichero
	# (duración, códecs, streams) al incrustar metadatos o portada. Va como
	# `lib*.so` por lo mismo que el resto: Android solo ejecuta desde
	# `nativeLibraryDir`, y Android solo extrae ahí los ficheros con ese nombre.
	install -m 755 "$PREFIX/bin/ffprobe" "$WORK/out/libffprobe.so"
	"$STRIP" --strip-all "$WORK/out/libffprobe.so"

	# quickjs: resuelve los retos de YouTube.
	install -m 755 "$SRC/quickjs/qjs" "$WORK/out/libquickjs.so"

	# El stdlib de Python va comprimido y se descomprime en el primer arranque:
	# las extensiones (`_ssl.so`, `_socket.so`…) tienen que ser ficheros de
	# verdad, no entradas de un zip.
	#   - Se limpian los símbolos de las extensiones (28 MB → 16 MB).
	#   - Fuera lo que aquí no sirve: el motor de la consola gráfica, los entornos
	#     virtuales, la biblioteca estática de incrustación (52 MB) y el bytecode
	#     de los niveles de optimización `-O` (que no se usan).
	find "$PREFIX/lib/python$PY_SHORT" -name '*.so' -exec "$STRIP" --strip-all {} +
	rm -rf "$PREFIX/lib/python$PY_SHORT"/{idlelib,tkinter,turtledemo,ensurepip,venv,lib2to3}
	rm -rf "$PREFIX/lib/python$PY_SHORT"/config-*
	find "$PREFIX/lib/python$PY_SHORT" -name '*.opt-*.pyc' -delete
	# Se deja el bytecode normal sí: ahorra compilar el stdlib en el teléfono la
	# primera vez que se lanza el motor.
	#
	# Certificados raíz de TLS: sin ellos Python arranca pero rechaza cualquier
	# HTTPS («unable to get local issuer certificate»), y yt-dlp no puede ni
	# hablar con YouTube. `openssl` los deja en su `openssldir`, que no se copia
	# al APK, así que se traen aparte con el paquete de `certifi` y se marcan con
	# `SSL_CERT_FILE` al descomprimir (ver `engine::prepare_python`).
	fetch "https://curl.se/ca/cacert.pem" cacert.pem
	install -D -m 644 "$SRC/cacert.pem" "$PREFIX/lib/python$PY_SHORT/cacert.pem"

	rm -f "$WORK/out/libpython-stdlib.so"
	(cd "$PREFIX/lib" && zip -q -r "$WORK/out/libpython-stdlib.so" "python$PY_SHORT")

	# yt-dlp viaja también dentro del APK: así la app funciona nada más
	# instalarla. La copia gestionada —la que se actualiza sola— manda sobre
	# esta. Con `YTDLP_VERSION` se puede fijar una versión concreta.
	local ytdlp_url="https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp"
	if [ -n "${YTDLP_VERSION:-}" ]; then
		ytdlp_url="https://github.com/yt-dlp/yt-dlp/releases/download/$YTDLP_VERSION/yt-dlp"
	fi
	rm -f "$SRC/yt-dlp"
	fetch "$ytdlp_url" yt-dlp
	install -m 644 "$SRC/yt-dlp" "$WORK/out/libyt-dlp.so"

	cp "$WORK/out"/lib*.so "$JNI_DIR/"
	du -h "$JNI_DIR"/lib*.so
}

steps=("$@")
[ ${#steps[@]} -eq 0 ] && steps=(openssl lame opus vorbis bzip2 xz zstd ffmpeg cpython quickjs package)
mkdir -p "$SRC" "$PREFIX" "$STAMPS"
for step in "${steps[@]}"; do
	"step_$step"
done
log "Listo"
