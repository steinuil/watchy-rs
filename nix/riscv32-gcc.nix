{
  stdenv,
  fetchzip,
  zlib,
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
  name = "esp-riscv32-gcc";
  version = "14.2.0_20241119";

  src = fetchzip {
    url = "https://github.com/espressif/crosstool-NG/releases/download/esp-${finalAttrs.version}/riscv32-esp-elf-${finalAttrs.version}-${targetArch}.tar.xz";
    hash = "sha256-67O34FYUnVG2nfkfQj2yH874qDSYx4F/16xxPi0kNbY=";
  };

  nativeBuildInputs = [ autoPatchelfHook ];
  buildInputs = [ zlib ];

  outputs = [ "out" ];

  installPhase = ''
    mkdir -p $out
    cp -r ./* $out/
  '';

  # preFixup = ''
  #   patchelf \
  #     --set-interpreter "$(cat $NIX_CC/nix-support/dynamic-linker)" \
  #     $out/bin/*
  # '';
})
