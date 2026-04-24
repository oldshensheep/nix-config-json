{
  lib,
  meson,
  ninja,
  nix,
  pkg-config,
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
  ];

  buildInputs = [
    nix.dev
  ];

  meta = {
    description = "Nix plugin that lazily serializes values to JSON";
    homepage = "https://github.com/oldshensheep/nix-plugins";
    platforms = lib.platforms.all;
  };
}
