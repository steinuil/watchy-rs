{
  stdenv,
  fetchzip,
}:
stdenv.mkDerivation (finalAttrs: {
  name = "esp-rust-src";
  version = "1.86.0.0";

  src = fetchzip {
    url = "https://github.com/esp-rs/rust-build/releases/download/v${finalAttrs.version}/rust-src-${finalAttrs.version}.tar.xz";
    hash = "sha256-A++Q0Cd2x5EOb3NRT2iwzzsPR9g8cv/ZVQY+QkEJOAk=";
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
