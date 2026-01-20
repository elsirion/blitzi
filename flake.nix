{
  description = "CandyPi - Lightning-paid candy dispenser for Raspberry Pi";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" ] (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        lib = nixpkgs.lib;

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [
            "rust-src"
            "rust-analyzer"
          ];
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

          # Disable fortify to avoid GCC warnings-as-errors in aws-lc-sys
          hardeningDisable = [ "fortify" ];

          buildAndTestSubdir = null;
          cargoBuildFlags = [
            "--bin"
            "blitzid"
          ];

          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";

          # Wrap CC to disable warnings that cause aws-lc-sys build to fail
          preBuild = ''
            export CC_WRAPPER="$NIX_BUILD_TOP/cc-wrapper"
            cat > "$CC_WRAPPER" << 'WRAPPER'
            #!/bin/sh
            exec ${pkgs.stdenv.cc}/bin/cc -Wno-error=stringop-overflow -Wno-error=array-bounds -Wno-error=restrict "$@"
            WRAPPER
            chmod +x "$CC_WRAPPER"
            export CC="$CC_WRAPPER"
          '';

          "ROCKSDB_${build_arch_underscores}_STATIC" = "true";
          "ROCKSDB_${build_arch_underscores}_LIB_DIR" = "${rocksdb}/lib/";

          meta = with lib; {
            description = "Blitzi Lightning REST API daemon";
            homepage = "https://github.com/elsirion/blitzi";
            license = licenses.mit;
            maintainers = [ ];
          };
        };

        blitzid-image = pkgs.dockerTools.buildLayeredImage {
          name = "blitzid";
          contents = [
            blitzidPackage
            pkgs.bash
            pkgs.coreutils
            pkgs.curl
          ]
          ++ pkgs.lib.optionals pkgs.stdenv.isLinux [ pkgs.busybox ];

          config = {
            Cmd = [
              "${blitzidPackage}/bin/blitzid"
            ];
          };
        };
      in
      {
        packages = {
          default = blitzidPackage;
          blitzid = blitzidPackage;
          inherit blitzid-image;
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            stdenv.cc # Include nix cc wrapper which respects NIX_CFLAGS_COMPILE
          ];

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

          # Disable warnings that cause aws-lc-sys build to fail in release mode
          NIX_CFLAGS_COMPILE = "-Wno-error=stringop-overflow -Wno-error=array-bounds -Wno-error=restrict";
        };
      }
    );
}
