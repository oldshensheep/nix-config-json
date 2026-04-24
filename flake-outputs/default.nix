{ self, nixpkgs, ... }@inputs:
let
  systems = [
    "x86_64-linux"
    "aarch64-linux"
  ];
  forSystems = f: nixpkgs.lib.genAttrs systems (system: f system);
  pkgsFor = system: import nixpkgs { inherit system; };

in
{
  packages = forSystems (
    system:
    let
      pkgs = pkgsFor system;
      plugin = pkgs.callPackage ./libnix-value-json.nix { };
      jsonDiff = pkgs.callPackage ./json-diff.nix { };
      lib = pkgs.lib;
      evalScript = pkgs.writeShellApplication {
        runtimeInputs = [ pkgs.nix ];
        name = "nix-value-json-script";
        text = ''
          HOST=$1
          nix eval --plugin-files ${lib.getLib plugin}/lib/libnix-value-json.so --raw \
            --apply ${lib.escapeShellArg (builtins.readFile ./eval.nix)} "$HOST"
        '';
      };
    in
    {
      libnix-value-json = plugin;
      json-diff = jsonDiff;
      default = evalScript;
    }
  );

  nixosConfigurations = {
    a = import ./hostA.nix inputs;
    b = import ./hostB.nix inputs;
  };
  devShells = forSystems (
    system:
    let
      pkgs = pkgsFor system;
      nixPkg = pkgs.nix;
    in
    {
      default = pkgs.mkShell {
        packages =
          (with pkgs; [
            pkg-config
            python3
            meson
            ninja
            gcovr
          ])
          ++ [
            nixPkg.dev
            nixPkg
          ];
      };
    }
  );
}
