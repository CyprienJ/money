#!/usr/bin/env bash
# Downloads the PDFium library used to read PDF statements into src-tauri/pdfium/.
#
# The version is pinned to the PDFium API targeted by pdfium-render (feature pdfium_7881),
# and each archive is checked against its SHA-256 before extraction. Tauri bundles the
# library as a resource (tauri.linux.conf.json, tauri.windows.conf.json).
#
# Usage: tools/fetch_pdfium.sh [linux-x64 | win-x64]   (default: the current system)
set -euo pipefail

VERSION="chromium/7881"
declare -A SHA256=(
  [linux-x64]=1470e21b8b4a3b4ad7f85684e2da11d94f3b69a86d81dee11b9b6709d927ac1d
  [win-x64]=73cc0de638ac2095e7445bf56a38200a5b7c7ca0e9f4ba144598f2457377ac08
)
declare -A LIBRARY=(
  [linux-x64]=lib/libpdfium.so
  [win-x64]=bin/pdfium.dll
)

platform="${1:-}"
if [[ -z "$platform" ]]; then
  case "$(uname -s)" in
    Linux) platform=linux-x64 ;;
    MINGW* | MSYS* | CYGWIN*) platform=win-x64 ;;
    *) echo "Unsupported system: $(uname -s)" >&2; exit 1 ;;
  esac
fi
if [[ -z "${SHA256[$platform]:-}" ]]; then
  echo "Unknown platform: $platform (expected linux-x64 or win-x64)" >&2
  exit 1
fi

root="$(cd "$(dirname "$0")/.." && pwd)"
target="$root/src-tauri/pdfium"
library="${LIBRARY[$platform]}"
if [[ -f "$target/$(basename "$library")" && -f "$target/VERSION" && "$(cat "$target/VERSION")" == "$VERSION" ]]; then
  echo "PDFium $VERSION already in $target"
  exit 0
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
archive="$work/pdfium.tgz"
url="https://github.com/bblanchon/pdfium-binaries/releases/download/${VERSION/\//%2F}/pdfium-$platform.tgz"
echo "Downloading $url"
curl --fail --silent --show-error --location --output "$archive" "$url"

actual="$(sha256sum "$archive" | cut -d ' ' -f 1)"
if [[ "$actual" != "${SHA256[$platform]}" ]]; then
  echo "Checksum mismatch for pdfium-$platform.tgz: got $actual" >&2
  exit 1
fi

tar -xzf "$archive" -C "$work" "$library" LICENSE
mkdir -p "$target"
cp "$work/$library" "$target/"
cp "$work/LICENSE" "$target/LICENSE"
echo "$VERSION" > "$target/VERSION"
echo "PDFium $VERSION installed in $target"
