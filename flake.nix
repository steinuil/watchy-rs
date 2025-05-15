{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";

    fenix.url = "github:nix-community/fenix/monthly";
    fenix.inputs.nixpkgs.follows = "nixpkgs";

    naersk.url = "github:nix-community/naersk";
    naersk.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      naersk,
      fenix,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };

        toolchain =
          with fenix.packages.${system};
          combine [
            (complete.withComponents [
              "cargo"
              "clippy"
              "rust-src"
              "rustc"
              "rustfmt"
            ])
            rust-analyzer
          ];

        naerskBuildPackage =
          (naersk.lib.${system}.override {
            cargo = toolchain;
            rustc = toolchain;
          }).buildPackage;

        packages = rec {
          esp-riscv32-gcc = pkgs.callPackage ./nix/riscv32-gcc.nix { };
          esp-xtensa-gcc = pkgs.callPackage ./nix/xtensa-gcc.nix { };
          esp-rust-src = pkgs.callPackage ./nix/rust-src.nix { };
          esp-rust = pkgs.callPackage ./nix/rust.nix { inherit esp-rust-src; };
        };
      in
      {
        # defaultPackage = naerskBuildPackage {
        #   src = ./.;
        # };
        #
        inherit packages;

        devShell = pkgs.mkShell {
          # nativeBuildInputs = [ toolchain ];

          buildInputs = with pkgs; [
            cargo-espflash
            lldb
            packages.esp-xtensa-gcc
            # packages.esp-riscv32-gcc
            packages.esp-rust
            rustup
          ];
        };
      }
    );
}
