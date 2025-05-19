{
  stdenv,
  fetchzip,
}:
stdenv.mkDerivation (finalAttrs: {
  name = "esp-rust-src";
  version = "1.97.0.0";

  src = fetchzip {
    url = "https://github.com/esp-rs/rust-build/releases/download/v${finalAttrs.version}/rust-src-${finalAttrs.version}.tar.xz";
    hash = "sha256-0Q/hfXD0R1pVzTgS0N8X2D3ZvPYX4y4k/EmxPy2474k=";
  };

  patchPhase = ''
    patchShebangs ./install.sh
  '';

  dontFixup = true;

  installPhase = ''
    mkdir -p $out
    ./install.sh --destdir=$out --prefix=""
  '';
})
