{ lib
, darwin
, stdenv
, openssl
, pkg-config
, rustPlatform
,
}:
let
  cargoFile = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package;
in
rustPlatform.buildRustPackage {

  pname = cargoFile.name; # The name of the package
  version = cargoFile.version; # The version of the package

  # You can use lib here to make a more accurate source
  # this can be nice to reduce the amount of rebuilds
  # but thats out of scope for this post
  src = ./.; # The source of the package

  # The lock file of the package, this can be done in other ways
  # like cargoHash, we are not doing it in this case because this
  # is much simpler, especially if we have access to the lock file
  # in our source tree
  cargoLock.lockFile = ./Cargo.lock;

  # The runtime dependencies of the package
  buildInputs =
    [ openssl ]
    ++ lib.optionals stdenv.isDarwin (
      with darwin.apple_sdk.frameworks;
      [
        Security
        CoreFoundation
        SystemConfiguration
      ]
    );

  # programs and libraries used at build-time that, if they are a compiler or
  # similar tool, produce code to run at run-time—i.e. tools used to build the new derivation
  nativeBuildInputs = [ pkg-config ];

  meta = {
    license = lib.licenses.mit;
    mainProgram = cargoFile.name;
  };
}
