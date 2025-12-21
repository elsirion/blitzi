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

        rocksdb = pkgs.rocksdb_8_11.override { enableLiburing = false; };

        blitzidPackage = pkgs.rustPlatform.buildRustPackage {
          pname = "blitzid";
          version = "0.3.0";

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = with pkgs; [
            pkg-config
            cmake
            clang
            rustToolchain
          ];

          buildInputs = [ rocksdb ];

          buildAndTestSubdir = null;
          cargoBuildFlags = [ "--bin" "blitzid" ];

          # Fix for aws-lc-sys compilation with newer glibc (2.40+)
          # aws-lc-sys triggers -Werror=stringop-overflow in glibc's memcpy checks
          # This is a known issue with aws-lc-sys and can be removed when the dependency is updated
          NIX_CFLAGS_COMPILE = "-Wno-error=stringop-overflow";

          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          "ROCKSDB_${build_arch_underscores}_STATIC" = "true";
          "ROCKSDB_${build_arch_underscores}_LIB_DIR" = "${rocksdb}/lib/";

          meta = with lib; {
            description = "Blitzi Lightning REST API daemon";
            homepage = "https://github.com/elsirion/blitzi";
            license = licenses.mit;
            maintainers = [ ];
          };
        };
      in
      {
        packages = {
          blitzid = blitzidPackage;
          default = blitzidPackage;
        };

        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            rust-bin.nightly.latest.rustfmt
            pkg-config
            cmake
            clang
            llvmPackages.libclang
            llvmPackages.libcxxClang
          ];

          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          RUSTFMT = "${pkgs.rust-bin.nightly.latest.rustfmt}/bin/rustfmt";
          "ROCKSDB_${build_arch_underscores}_STATIC" = "true";
          "ROCKSDB_${build_arch_underscores}_LIB_DIR" = "${rocksdb}/lib/";
        };
      });
}
