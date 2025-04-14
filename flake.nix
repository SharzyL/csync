{
  description = "csync";

  inputs = {
    nixpkgs.url = "nixpkgs";
    flake-utils.url = "flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }@inputs:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        rec {
          legacyPackages = pkgs;

          defaultPackage = pkgs.rustPlatform.buildRustPackage {
            pname = "csync";
            version = "0.1.0";
            src = with pkgs.lib.fileset; toSource {
              root = ./.;
              fileset = fileFilter (file: file.name != "flake.nix") ./.;
            };
            useFetchCargoVendor = true;
            cargoHash = "sha256-CA8fMxv86iAcMkdoelUCSEy4RM5VEdjKsiGS7N3cq5Q=";
          };

          devShell = defaultPackage.overrideAttrs (_: { });
        }
      )
    // {
      inherit inputs; # for easier introspection via nix repl
    };
}
