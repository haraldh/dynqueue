{ lib
, rustPlatform
}:
let
  cargoFile = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package;
in
rustPlatform.buildRustPackage {

  pname = cargoFile.name;
  version = cargoFile.version;

  src = ./.; # The source of the package

  # Use the checked-in lock file instead of a source-hash for simplicity.
  cargoLock.lockFile = ./Cargo.lock;

  meta = {
    # This is a pure Rust library without external C/system dependencies, so no
    # extra buildInputs or nativeBuildInputs (e.g. openssl, pkg-config) are needed.
    license = lib.licenses.mit;
  };
}
