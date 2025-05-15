{
  stdenv,
  fetchzip,
  zlib,
  autoPatchelfHook,
  esp-rust-src,
}:
let
  targetArch =
    {
      x86_64-linux = "x86_64-unknown-linux-gnu";
      aarch64-linux = "aarch64-unknown-linux-gnu";
      # aarch64-darwin = "aarch64-dapple-darwin";
    }
    .${stdenv.targetPlatform.system};
in
stdenv.mkDerivation (finalAttrs: {
  name = "esp-rust";
  version = "1.86.0.0";

  src = fetchzip {
    url = "https://github.com/esp-rs/rust-build/releases/download/v${finalAttrs.version}/rust-${finalAttrs.version}-${targetArch}.tar.xz";
    hash = "sha256-NEKcsHbgh0UwESLg5zR2o3ACIwMk4DiEfgzidctTJoY=";
  };

  nativeBuildInputs = [ autoPatchelfHook ];
  buildInputs = [
    zlib
    stdenv.cc.cc.lib
  ];

  patchPhase = ''
    patchShebangs ./install.sh
  '';

  outputs = [ "out" ];

  installPhase = ''
    mkdir -p $out
    ./install.sh --destdir=$out --prefix="" --disable-ldconfig --without=rust-docs-json-preview,rust-docs
    cp -r ${esp-rust-src}/lib/rustlib/src $out/lib/rustlib/src
  '';
})
