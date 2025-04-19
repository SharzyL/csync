{ lib
, installShellFiles
, rustPlatform
}:

let
  extractCargoVersion = cargoFile:
    builtins.head (builtins.match ".*\nversion *= *\"([[:digit:].]+)\" *\n.*"
      (builtins.readFile cargoFile)
    );
in

rustPlatform.buildRustPackage {
  pname = "csync";
  version = extractCargoVersion ./Cargo.toml;

  src = with lib.fileset; toSource {
    root = ./.;
    fileset = fileFilter (file: file.name != "flake.nix") ./.;
  };

  nativeBuildInputs = [
    installShellFiles
  ];

  preBuild = ''
    mkdir -p $TMPDIR/completions
    export COMPLETION_OUT_DIR=$TMPDIR/completions
  '';

  postInstall = ''
    installShellCompletion "$COMPLETION_OUT_DIR"/{csync.fish,csync.bash,_csync}
  '';

  useFetchCargoVendor = true;
  cargoHash = "sha256-9CW4m4poQTWraFAWrvUOP/c7PBvNQLB8EOEAVRSbVig=";
}
