{
  description = "CandyPi - Lightning-paid candy dispenser for Raspberry Pi";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" ] (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        lib = nixpkgs.lib;

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };

        build_arch_underscores =
          lib.strings.replaceStrings [ "-" ] [ "_" ]
            pkgs.stdenv.buildPlatform.config;
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            pkg-config
            cmake
            clang
            llvmPackages.libclang
            llvmPackages.libcxxClang
          ];

          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          "ROCKSDB_${build_arch_underscores}_STATIC" = "true";
          "ROCKSDB_${build_arch_underscores}_LIB_DIR" = "${
            pkgs.rocksdb_8_11.override { enableLiburing = false; }
          }/lib/";
        };
      });
}
