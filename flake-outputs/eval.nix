host:
let
  config = host.config;
  valueToForce = config.system.build.toplevel.drvPath;
  valueToPrint = builtins.lazyToJSON config [
    "home-manager.extraSpecialArgs.nixosConfig"
    "assertions"
  ];
in
builtins.seq valueToForce valueToPrint
