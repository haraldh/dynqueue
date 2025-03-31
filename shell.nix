{ clippy
, rustfmt
, callPackage
, rust-analyzer
,
}:
let
  mainPkg = callPackage ./default.nix { };
in
mainPkg.overrideAttrs (prev: {
  nativeBuildInputs = [
    # Additional Rust tooling
    clippy
    rustfmt
    rust-analyzer
  ] ++ (prev.nativeBuildInputs or [ ]);
})

