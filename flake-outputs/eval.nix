host:
let
  config = host.config;
  system = config.system.build.toplevel;
in
builtins.seq system (
  builtins.lazyToJSON config [
    "home-manager.extraSpecialArgs.nixosConfig"
    "assertions"
  ]
)
