{ nixpkgs, ... }:
nixpkgs.lib.nixosSystem {
  modules = [
    ./hostBase.nix
    (
      { lib, ... }:
      {
        environment.defaultPackages = lib.mkForce [ ];
      }
    )
  ];
}
