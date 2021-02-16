project('gpower-tweaks',
    version: '0.1.0-dev',
    meson_version: '>= 0.53.0',
    license: 'GPL-3.0-or-later',
    default_options: ['buildtype=release']
)

name       = 'GPower Tweaks'
app_id     = 'net.gourlaysama.GPowerTweaks'
vcs_tag    = run_command('git', 'rev-parse', '--short', 'HEAD').stdout().strip()
localedir  = join_paths(get_option('prefix'), get_option('localedir'))
datadir    = get_option('datadir')
pkgdatadir = join_paths(get_option('prefix'), datadir, meson.project_name())

dependency('glib-2.0')
dependency('gio-2.0', version: '>= 2.46')
dependency('gtk+-3.0', version: '>= 3.22')

cargo = find_program('cargo')
rustc = find_program('rustc', version: '>= 1.43')
find_program('awk')
find_program('find')
find_program('tar')

rust_flags = run_command('printenv', 'RUSTFLAGS').stdout().strip()

sources = files(
    'Cargo.toml',
    'Cargo.lock',
    'src/db/mod.rs',
    'src/db/parsers.rs',
    'src/pci/mod.rs',
    'src/usb/mod.rs',
    'src/app.rs',
    'src/fs.rs',
    'src/lib.rs',
    'src/main.rs',
)

cargo_helper = find_program(join_paths(meson.source_root(), 'build-aux/cargo.sh'))
main = custom_target(
  meson.project_name(),
  build_by_default: true,
  input: sources,
  output: meson.project_name(),
  console: true,
  install: true,
  install_dir: get_option('bindir'),
  command: [
    cargo_helper,
    cargo,
    meson.build_root(),
    meson.source_root(),
    '@OUTPUT@',
    get_option('debug').to_string(),
    rust_flags
  ]
)

install_data('data/@0@.desktop'.format(app_id),
  install_dir: join_paths(datadir, 'applications')
)

configure_file(
  input: 'data/@0@.metainfo.xml.in'.format(app_id),
  output : '@BASENAME@',
  configuration : {
    'app_id' : app_id,
  },
  install_dir: join_paths(datadir, 'metainfo')
)

bin_dist_helper = find_program(join_paths(meson.source_root(), 'build-aux/bin-dist.sh'))
run_target('bin-dist',
  command: [
    bin_dist_helper,
    meson.project_name(),
    # additional files to package manually
    'README.md',
    'COPYING'
  ],
  depends: [main],
)

summary({
    'cargo': run_command(cargo, '-V').stdout().strip(),
    'rustc': run_command(rustc, '-V').stdout().strip(),
    'RUSTFLAGS': rust_flags,
    'debug': get_option('debug')
}, section: 'Rust')