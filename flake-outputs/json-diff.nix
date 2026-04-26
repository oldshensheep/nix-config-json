{
  lib,
  rustPlatform,
}:
rustPlatform.buildRustPackage {
  pname = "json-diff";
  version = "0.1.0";

  src = ../json-diff;

  cargoLock.lockFile = ../json-diff/Cargo.lock;

  meta = {
    description = "Git-style JSON structural diff CLI";
    homepage = "https://github.com/oldshensheep/nix-value-json";
    mainProgram = "json-diff";
    platforms = lib.platforms.all;
  };
}
