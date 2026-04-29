# nix-value-json

Tools for serializing Nix values to JSON and comparing the resulting data.

The main piece is `libnix-value-json`, a Nix plugin that adds:

```nix
builtins.lazyToJSON value skipAttrPaths
```

It serializes already-evaluated Nix values to JSON without forcing thunks inside
the value, except attrsets with a `type` key. This is useful when inspecting
large NixOS configurations where a normal full evaluation or `builtins.toJSON`
would force too much.

Note that the builtin output depends on Nix’s internal evaluation implementation. The same value may produce different results, and the project may change the output format of a value, so please don’t use it for other purposes.

## Packages

This flake exposes:

- `.#libnix-value-json`: shared library plugin for Nix
- `.#json-diff`: Git-style structural JSON diff CLI
- `.#default`: helper script that evaluates a NixOS configuration through
  `builtins.lazyToJSON`

## Usage

Generate JSON from the hosts:

```bash
nix run github:oldshensheep/nix-value-json# -- .#nixosConfigurations.a > a.json
nix run github:oldshensheep/nix-value-json# -- .#nixosConfigurations.b > b.json
```

Diff them:

```bash
nix run github:oldshensheep/nix-value-json#json-diff -- a.json b.json
```

If the output is mostly Nix store path churn, use `json-diff -S hash` to
ignore store path hash changes, or `json-diff -S full` to ignore any path
starting with `/nix/store/<hash>-...`.

Example output after `nix flake update` and add nushell to `home.packages` with `-S full`:

```diff
@ home-manager.sharedModules[25]
+ "«github:nix-community/nix-index-database/b8eb7acee0f7604fe1bf6a5b3dcf5254369180fa?narHash=sha256-yVJbd07ortDRAttDFmDV5p220aOLTHgVAx//0nW/xW8%3D»/home-manager-module.nix"

@ home-manager.sharedModules[25]
- "«github:nix-community/nix-index-database/c43246d4e9e506178b69baed075d797ec2d873e2?narHash=sha256-oHVcvP2Ahhj1KUsEzp%2B2BQF55/r5VSa3QxdPdwE1p00%3D»/home-manager-module.nix"

@ home-manager.users.mio.home.extraDependencies[146]
+ "<derivation: nushell-0.112.2-fish-completions>"

@ home-manager.users.mio.home.packages[150]
+ "<derivation: nushell-0.112.2>"

Stats: added 3, deleted 1, ignored 20
```

Use the plugin directly:

```bash
nix build github:oldshensheep/nix-value-json#libnix-value-json

nix eval \
  --plugin-files ./result/lib/libnix-value-json.so \
  --raw \
  --expr 'builtins.lazyToJSON 1 []'
```

Note: the plugin uses Nix's C++ API, which is not a stable public interface
across all Nix versions. If it fails with your Nix version, please report it. Tested on Nix 2.34.6

## NixOS Module Integration

One way to make the evaluated configuration available on a system is to add a
small module layer that writes the lazy JSON output into the system closure:

```nix
nixosConfigurations.<host> =
  let
    layer0 = nixpkgs.lib.nixosSystem {
      modules = [
        ./configuration.nix
      ];
    };
    final = layer0.extendModules {
      modules = [
        (
          { pkgs, ... }:
          {
            environment = {
              pathsToLink = [ "/share/nixos-config" ];
              systemPackages =
                let
                  valueToPrint = layer0.config;
                  valueToForce = valueToPrint.system.build.toplevel.drvPath;
                  configJsonFile = pkgs.writeText "nixos-config.json" (
                    builtins.seq valueToForce (
                      builtins.lazyToJSON valueToPrint [
                        "home-manager.extraSpecialArgs"
                        "assertions"
                        "meta.doc"
                        # if you add your config to registry...
                        # "nix.registry.<name>.to.path"
                      ]
                    )
                  );
                in
                [
                  (pkgs.runCommandLocal "print-config" { } ''
                    install -D ${configJsonFile} $out/share/nixos-config/nixos-config.json
                  '')
                ];
            };
          }
        )
      ];
    };
  in
  final;
```

The plugin must be loaded while the configuration is evaluated. To load it
persistently on NixOS, add it to `nix.settings.plugin-files`:

```nix
{
  nix.settings.plugin-files = [
    "${lib.getLib inputs.nix-value-json.packages.${pkgs.stdenv.hostPlatform.system}.libnix-value-json}/lib/libnix-value-json.so"
  ];
}
```

For a one-off rebuild, pass the plugin path on the command line:

```bash
nix build github:oldshensheep/nix-value-json#libnix-value-json

nixos-rebuild switch --flake .#<host> --sudo --show-trace -L \
  --option plugin-files "result/lib/libnix-value-json.so"
```

After deploying multiple generations with this module, diff their exported
configuration JSON files from the system profile:

```bash
nix run github:oldshensheep/nix-value-json#json-diff -- \
  /nix/var/nix/profiles/system-123-link/sw/share/nixos-config/nixos-config.json \
  /run/current-system/sw/share/nixos-config/nixos-config.json
```

## `lazyToJSON` Behavior

`builtins.lazyToJSON` takes two arguments:

```nix
builtins.lazyToJSON value skipAttrPaths
```

`skipAttrPaths` is a list of dot-separated attr paths. Any current attr path
containing one of those filters is replaced with a placeholder instead of being
walked.

Special values are rendered as JSON strings:

- thunks: `"<thunk>"`
- blackholes: `"<potential infinite recursion>"`
- derivations: `"<derivation: name>"`, `"<derivation: unforced name>"`,
  `"<derivation: non-string name>"`, or `"<derivation: null>"`
- functions: `"<function>"`
- repeated containers: `"<repeated: path.to.first.value>"`
- recursive containers: `"<recursive: path.to.active.value>"`
- non-finite floats: `"<nan>"`, `"<inf>"`, `"<-inf>"`

Strings are validated as UTF-8 and escaped as JSON strings.

## Development

Enter the dev shell:

```bash
nix develop .
```

The dev shell is focused on the Nix plugin workflow. It does not include
`rustc` or Cargo for working on `json-diff`.

Run all plugin behavior tests:

```bash
cd libnix-value-json
meson setup builddir .
meson test -C builddir --print-errorlogs
```

Generate coverage for `libnix-value-json/plugin.cpp`:

```bash
cd libnix-value-json
meson setup builddir-coverage . -Db_coverage=true -Dbuildtype=debug
meson test -C builddir-coverage --print-errorlogs
meson compile -C builddir-coverage plugin-coverage-html
```

The coverage report is written to:

```text
libnix-value-json/builddir-coverage/coverage.html
```
