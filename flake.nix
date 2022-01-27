{
  description = "Iterator-based bitcoin indexing toolkit";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";

    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, naersk, nixpkgs, flake-utils, flake-compat, fenix }:
    flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = nixpkgs.legacyPackages."${system}";
      fenix-pkgs = fenix.packages.${system};
      fenix-channel = fenix-pkgs.stable;
      naersk-lib = naersk.lib."${system}".override {
        inherit (fenix-pkgs.minimal) cargo rustc;
      };
    in rec {
      packages.block-iter = naersk-lib.buildPackage {
        name = "block-iter";
        version = "0.1.0";
        src = ./.;
        buildInputs = [ pkgs.rocksdb pkgs.llvmPackages.clang pkgs.pkgconfig ];
      };

      defaultPackage = self.packages.${system}.block-iter;
      defaultApp = self.packages.${system}.block-iter;

      # `nix develop`
      devShell = pkgs.mkShell
        {
          inputsFrom = builtins.attrValues self.packages.${system};
          buildInputs = [ pkgs.libsodium pkgs.lzma pkgs.openssl ];
          nativeBuildInputs = (with pkgs;
            [
              pkgconfig
              fenix-pkgs.rust-analyzer
              fenix-channel.rustfmt
              fenix-channel.rustc
              rocksdb
              llvmPackages.clang
            ]);
          RUST_SRC_PATH = "${fenix-channel.rust-src}/lib/rustlib/src/rust/library";
          LIBCLANG_PATH = "${pkgs.llvmPackages.clang.cc.lib}/lib";
        };
  });
}
