{
  lib,
  meson,
  ninja,
  nix,
  pkg-config,
  python3,
  stdenv,
}:
stdenv.mkDerivation {
  pname = "libnix-value-json";
  version = "0.1.0";

  src = ../libnix-value-json;

  nativeBuildInputs = [
    meson
    ninja
    pkg-config
    python3
  ];

  nativeCheckInputs = [
    nix
  ];

  buildInputs = [
    nix.dev
  ];

  doCheck = true;

  meta = {
    description = "Nix plugin that lazily serializes values to JSON";
    homepage = "https://github.com/oldshensheep/nix-value-json";
    platforms = lib.platforms.all;
  };
}
