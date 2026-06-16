#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "usage: $0 <deb-dir> <output-dir>" >&2
  exit 2
fi

deb_dir="$1"
out_dir="$2"
pool_dir="$out_dir/pool/main/w/worldcup26"

mkdir -p "$pool_dir"
find "$deb_dir" -type f -name '*.deb' -exec cp {} "$pool_dir/" \;

if ! find "$pool_dir" -type f -name '*.deb' | grep -q .; then
  echo "no .deb files found under $deb_dir" >&2
  exit 1
fi

for arch in amd64 arm64 armhf; do
  dists_dir="$out_dir/dists/stable/main/binary-$arch"
  mkdir -p "$dists_dir"
  (
    cd "$out_dir"
    dpkg-scanpackages --arch "$arch" --multiversion pool /dev/null | gzip -9c > "dists/stable/main/binary-$arch/Packages.gz"
  )
done

cat > "$out_dir/dists/stable/Release" <<RELEASE
Origin: WorldCup-2026
Label: WorldCup-2026
Suite: stable
Codename: stable
Architectures: amd64 arm64 armhf
Components: main
Description: WorldCup26 release packages
RELEASE
