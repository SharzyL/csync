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
          overlay = final: prev: {
            csync = final.callPackage ./pkg.nix { };
          };
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ overlay ];
          };
        in
        {
          legacyPackages = pkgs;

          defaultPackage = pkgs.csync;

          devShell = pkgs.csync.overrideAttrs (_: { });
        }
      )
    // {
      inherit inputs; # for easier introspection via nix repl
    };
}
