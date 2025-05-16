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
  version = "14.2.0_20241119";

  src = fetchzip {
    url = "https://github.com/espressif/crosstool-NG/releases/download/esp-${finalAttrs.version}/xtensa-esp-elf-${finalAttrs.version}-${targetArch}.tar.xz";
    hash = "sha256-pX2KCnUoGZtgqFmZEuNRJxDMQgqYYPRpswL3f3T0nWE=";
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
