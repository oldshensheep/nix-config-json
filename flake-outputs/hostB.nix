{ nixpkgs, ... }:
nixpkgs.lib.nixosSystem {
  modules = [
    ./hostBase.nix
    (
      { pkgs, ... }:
      {
        environment.systemPackages = [
          pkgs.firefox
          pkgs.thunderbird
        ];
      }
    )
  ];
}
