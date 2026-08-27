#!/usr/bin/env bash
# Collect the sideload APK, check 16 KB ELF LOAD alignment, and print size.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
android_root="$root/src-tauri/gen/android"
out_dir="$root/release-artifacts"
mkdir -p "$out_dir"

apk=""
for candidate in \
  "$android_root/app/build/outputs/apk/universal/release/app-universal-release.apk" \
  "$android_root/app/build/outputs/apk/arm64/release/app-arm64-release.apk"; do
  if [[ -f "$candidate" ]]; then
    apk="$candidate"
    break
  fi
done

if [[ -z "$apk" ]]; then
  echo "No aarch64/universal release APK found" >&2
  find "$android_root" -type f -name "*.apk" | sort || true
  exit 1
fi

cp "$apk" "$out_dir/$(basename "$apk")"
# Only the intended sideload APK — do not copy leftover ABI APKs.
find "$out_dir" -type f ! -name "$(basename "$apk")" -delete || true

echo "Collected: $out_dir/$(basename "$apk")"
ls -lh "$out_dir"

readelf_bin=""
if command -v llvm-readelf >/dev/null 2>&1; then
  readelf_bin="$(command -v llvm-readelf)"
elif command -v readelf >/dev/null 2>&1; then
  readelf_bin="$(command -v readelf)"
else
  ndk_home="${NDK_HOME:-${ANDROID_NDK_HOME:-}}"
  if [[ -n "$ndk_home" ]]; then
    readelf_bin="$(find "$ndk_home" -type f -name llvm-readelf | head -n 1 || true)"
  fi
fi
if [[ -z "$readelf_bin" ]]; then
  echo "llvm-readelf / readelf not found" >&2
  exit 1
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
unzip -q -o "$apk" "lib/arm64-v8a/*.so" -d "$tmp"
shopt -s nullglob
so_files=("$tmp"/lib/arm64-v8a/*.so)
if [[ "${#so_files[@]}" -eq 0 ]]; then
  echo "APK has no arm64-v8a native libraries" >&2
  unzip -l "$apk" | sed -n '/\.so$/p' || true
  exit 1
fi

failed=0
pebble_so_bytes=0
for so in "${so_files[@]}"; do
  size="$(wc -c < "$so")"
  echo
  echo "=== $(basename "$so")  $(du -h "$so" | cut -f1)  (${size} bytes) ==="
  if [[ "$(basename "$so")" == libpebble_lib.so ]]; then
    pebble_so_bytes="$size"
  fi
  "$readelf_bin" -l "$so"
  alignment="$("$readelf_bin" -l "$so" | awk '
    /LOAD/ {
      align=$NF
      print align
      if (align == "0x1000" || align == "4096" || align == "2**12") bad=1
      if (align == "0x4000" || align == "16384" || align == "2**14") good=1
    }
    END {
      if (NR == 0) exit 2
      if (bad) exit 1
      if (!good) exit 3
    }
  ')"
  align_status=$?
  echo "LOAD Align column: ${alignment:-<none>}"
  if [[ "$align_status" -eq 2 ]]; then
    echo "FAIL: no LOAD segments in $so" >&2
    failed=1
  elif [[ "$align_status" -eq 1 ]]; then
    echo "FAIL: $so has 4 KB (4096) LOAD alignment" >&2
    failed=1
  elif [[ "$align_status" -eq 3 ]]; then
    echo "FAIL: $so LOAD alignment is not 16384" >&2
    failed=1
  else
    echo "OK: LOAD alignment is 16384 (0x4000)"
  fi
done

if [[ "$failed" -ne 0 ]]; then
  exit 1
fi

# Unstripped debug pebble_lib.so was ~200–300 MB on device. Refuse that.
if [[ "$pebble_so_bytes" -gt 80000000 ]]; then
  echo "FAIL: libpebble_lib.so is ${pebble_so_bytes} bytes (unstripped debug?)" >&2
  exit 1
fi

apk_bytes="$(wc -c < "$apk")"
uncompressed_so="$(unzip -l "$apk" | awk '/\.so$/ { sum += $1 } END { print sum+0 }')"
echo
echo "=== Size report vs previous ~90 MB zip / ~300 MB on-device ==="
echo "APK zip:            $(du -h "$apk" | cut -f1) (${apk_bytes} bytes)"
echo "Uncompressed .so:   ${uncompressed_so} bytes"
echo "Installed estimate: APK unzipped native libs + dex/resources (was ~300 MB when the debug .so was unstripped)"
unzip -l "$apk" | awk '
  /\.so$/ { printf "  %10d  %s\n", $1, $4 }
'
echo "Artifact: pebble-android-apk / $(basename "$apk")"
