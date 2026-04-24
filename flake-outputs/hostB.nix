{ nixpkgs, ... }:
nixpkgs.lib.nixosSystem {
  modules = [
    ./hostBase.nix
    (
      { pkgs, ... }:
      {
        services.nginx.enable = true;
        environment.systemPackages = [
          pkgs.firefox
          pkgs.thunderbird
        ];
      }
    )
  ];
}
