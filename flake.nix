{
  description = "A static verifier for Rust, based on the Viper verification infrastructure.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "utils";
    };
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, naersk, rust-overlay, utils }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlay ];
        };

        rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain;

        naersk-lib = naersk.lib."${system}".override {
          cargo = rust;
          rustc = rust;
        };

        prusti-version = "${self.tag or "${self.lastModifiedDate}.${self.shortRev or "dirty"}"}";
      in rec {
        packages = {
          prusti = naersk-lib.buildPackage {
            name = "prusti";
            version = "${prusti-version}";
            root = ./.;
            buildInputs = [
              pkgs.pkg-config
              pkgs.wget
              pkgs.gcc
              pkgs.openssl
              pkgs.jdk11
              packages.viper
              packages.ow2_asm
            ];
            nativeBuildInputs = [
              pkgs.makeWrapper
            ];

            # prusti-contracts is not in the root workspace so naersk
            # doesn't copy it to the store for the dependency-only build
            # when `singleStep = false;`
            singleStep = true;

            override = _: {
              preBuild = ''
                export LD_LIBRARY_PATH="${pkgs.jdk11}/lib/openjdk/lib/server"
                export RUST_SYSROOT="${rust}"
                export VIPER_HOME="${packages.viper}/backends"
                export Z3_EXE="${pkgs.z3}/bin/z3"
                export ASM_JAR="${packages.ow2_asm}/asm.jar"
              '';
            };
            overrideMain = _: {
              postInstall = ''
                # naersk installs several extraneous executables like prusti_driver-153bce8fbb073a23
                rm -rf $out/bin
                mkdir $out/bin

                for exe in prusti-driver prusti-server-driver prusti-server prusti-rustc cargo-prusti; do
                  cp target/release/$exe $out/bin
                  wrapProgram $out/bin/$exe \
                    --set RUST_SYSROOT "$RUST_SYSROOT" \
                    --set VIPER_HOME "$VIPER_HOME" \
                    --set Z3_EXE "$Z3_EXE" \
                    --set JAVA_HOME "${pkgs.jdk11}/lib/openjdk" \
                    --suffix LD_LIBRARY_PATH : "${pkgs.jdk11}/lib/openjdk/lib/server"
                done

                mkdir $out/bin/deps
                cp prusti-contracts/target/verify/release/libprusti_contracts.rlib $out/bin
                cp prusti-contracts/target/verify/release/libprusti_std.* $out/bin
                cp prusti-contracts/target/verify/release/deps/libprusti_contracts_proc_macros-* $out/bin/deps
                cp prusti-contracts/target/verify/release/deps/libprusti_contracts-* $out/bin/deps
              '';
            };
          };

          viper = pkgs.stdenv.mkDerivation rec {
            pname = "viper-bin";
            version = "2022-10-31-1114";
            src = pkgs.fetchzip {
              url = "https://github.com/viperproject/viper-ide/releases/download/v-${version}/ViperToolsLinux.zip";
              hash = "sha256-4Cm8C/JRLTtXuAoAUg3991Buhx7pxpXIKLwBcHU4oyM=";
              stripRoot = false;
            };

            nativeBuildInputs = with pkgs; [
              autoPatchelfHook
            ];

            buildInputs = with pkgs; [
              stdenv.cc.cc
              krb5
              lttng-ust_2_12
              zlib
            ];

            dontConfigure = true;
            dontBuild = true;

            installPhase = ''
              runHook preInstall
              cp -r $src $out
              runHook postInstall
            '';
          };

          ow2_asm = pkgs.stdenv.mkDerivation rec {
            name = "asm";
            version = "3.3.1";
            src = pkgs.fetchurl {
              url = "https://repo.maven.apache.org/maven2/${name}/${name}/${version}/${name}-${version}.jar";
              hash = "sha256-wrOSdfjpUbx0dQCAoSZs2rw5OZvF4T1kK/LTRkSd9/M=";
            };
            dontUnpack = true;
            dontBuild = true;
            installPhase = ''
              mkdir $out
              cp ${src} $out/asm.jar
            '';
          };
        };

        checks = {
          # prusti-test = naersk-lib.buildPackage {
          #   name = "prusti-test";
          #   version = "${prusti-version}";
          #   root = ./.;
          #   checkInputs = [
          #     pkgs.pkg-config
          #     pkgs.wget
          #     pkgs.gcc
          #     pkgs.openssl
          #     pkgs.jdk11
          #     packages.viper
          #     packages.ow2_asm
          #   ];

          #   doCheck = true;

          #   override = _: {
          #     preBuild = ''
          #       export LD_LIBRARY_PATH="${pkgs.jdk11}/lib/openjdk/lib/server"
          #       export VIPER_HOME="${packages.viper}/backends"
          #       export Z3_EXE="${packages.viper}/z3/bin/z3"
          #       export ASM_JAR="${packages.ow2_asm}/asm.jar"
          #     '';
          #     preCheck = ''
          #       export RUST_SYSROOT="${rust}"
          #       export JAVA_HOME="${pkgs.jdk11}/lib/openjdk"
          #       export LD_LIBRARY_PATH="${pkgs.jdk11}/lib/openjdk/lib/server"
          #       export VIPER_HOME="${packages.viper}/backends"
          #       export Z3_EXE="${packages.viper}/z3/bin/z3"
          #     '';
          #   };
          # };

          prusti-simple-test = pkgs.runCommand "prusti-simple-test" {
            buildInputs = [
              defaultPackage
              rust
            ];
          }
          ''
            cargo new --name example $out/example
            sed -i '1s/^/use prusti_contracts::*;\n/;s/println.*$/assert!(true);/' $out/example/src/main.rs
            cargo-prusti --manifest-path=$out/example/Cargo.toml
          '';
        };

        defaultPackage = packages.prusti;
      }
    );
}
