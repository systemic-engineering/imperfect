{
  description = "imperfect";

  inputs = {
    nixpkgs.url     = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    flakes.url      = "github:systemic-engineering/flakes";
    flakes.inputs.nixpkgs.follows = "nixpkgs";
    flakes.inputs.flake-utils.follows = "flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils, flakes }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        rust = flakes.lib.${system}.rust;
      in {
        devShells.default = pkgs.mkShell {
          buildInputs = [
            pkgs.git pkgs.just pkgs.jq
            pkgs.openssl pkgs.zlib
            pkgs.gfortran
          ] ++ rust.rustTools
            ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isDarwin [
            pkgs.libiconv
          ];
          shellHook = ''
            export LANG=en_US.UTF-8
          '' + rust.rustHook;
        };
      });
}
