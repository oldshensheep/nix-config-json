project(
  'nix-value-json-plugin',
  ['c', 'cpp'],
  version: '0.1.0',
)

nix_expr = dependency('nix-expr', required: true)

plugin = shared_library(
  'nix-value-json',
  'plugin.cpp',
  dependencies: [nix_expr],
  install: true,
)
