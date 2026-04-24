{ lib, ... }:
{
  nixpkgs.hostPlatform = {
    system = "x86_64-linux";
  };
  fileSystems = {
    "/" = {
      device = "/dev/sda1";
      fsType = "ext4";
    };
    "/boot" = {
      device = "/dev/sda2";
      fsType = "fat32";
    };
  };
  boot.loader.systemd-boot.enable = true;
  system.stateVersion = lib.trivial.release;
}
