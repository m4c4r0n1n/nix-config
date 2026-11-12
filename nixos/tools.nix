{ rustPlatform }:

rustPlatform.buildRustPackage {
  pname = "nix-config-tools";
  version = "0.1.0";
  src = ./tools;
  cargoLock.lockFile = ./tools/Cargo.lock;
}
