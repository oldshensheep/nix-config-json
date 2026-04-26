project(
  'nix-value-json-plugin',
  ['c', 'cpp'],
  version: '0.1.0',
)

nix_expr = dependency('nix-expr', required: true)
python = import('python').find_installation('python3')
gcovr = find_program('gcovr', required: false)

plugin = shared_library(
  'nix-value-json',
  'plugin.cpp',
  dependencies: [nix_expr],
  install: true,
)

tests_dir = meson.current_source_dir() / 'tests'
test(
  'libnix-value-json',
  python,
  args: [
    tests_dir / 'libnix_value_json.py',
    plugin.full_path(),
    '--cases',
    tests_dir / 'libnix_value_json.csv',
  ],
  depends: plugin,
)

if gcovr.found()
  run_target(
    'plugin-coverage-html',
    command: [
      gcovr,
      '--root',
      meson.current_source_dir(),
      '--object-directory',
      meson.current_build_dir(),
      '--filter',
      meson.current_source_dir() / 'plugin.cpp',
      '--html-details',
      meson.current_build_dir() / 'coverage.html',
    ],
  )
endif
