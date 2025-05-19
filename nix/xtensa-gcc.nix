{
  stdenv,
  fetchzip,
  autoPatchelfHook,
}:
let
  targetArch =
    {
      x86_64-linux = "x86_64-linux-gnu";
      aarch64-linux = "aarch64-linux-gnu";
      aarch64-darwin = "aarch64-dapple-darwin";
    }
    .${stdenv.targetPlatform.system};
in
stdenv.mkDerivation (finalAttrs: {
  name = "esp-xtensa-gcc";
  version = "16.1.0_20260609";

  src = fetchzip {
    url = "https://github.com/espressif/crosstool-NG/releases/download/esp-${finalAttrs.version}/xtensa-esp-elf-${finalAttrs.version}-${targetArch}.tar.xz";
    hash = "sha256-D02nz89injwvi+CD8tE8j/xkp1YrS28XvAmqCd3Dm+A=";
  };

  nativeBuildInputs = [ autoPatchelfHook ];
  buildInputs = [
    stdenv.cc.cc.lib
  ];

  outputs = [ "out" ];

  installPhase = ''
    mkdir -p $out
    cp -r ./* $out/
  '';
})
