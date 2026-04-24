host:
let
  config = host.config;
  system = config.system.build.toplevel;
in
builtins.seq system (
  builtins.lazyToJSON host [
    "home-manager.extraSpecialArgs.nixosConfig"
  ]
)
