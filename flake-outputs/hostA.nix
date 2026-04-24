{ nixpkgs, ... }:
nixpkgs.lib.nixosSystem {
  modules = [
    ./hostBase.nix
    {
      services.pipewire.enable = true;
    }
  ];
}
