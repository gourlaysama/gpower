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
rustc = find_program('rustc', version: '>= 1.42')

rust_flags = run_command('printenv', 'RUSTFLAGS').stdout().strip()

sources = files(
    'Cargo.toml',
    'Cargo.lock',
    'src/usb/db.rs',
    'src/usb/mod.rs',
    'src/app.rs',
    'src/fs.rs',
    'src/lib.rs',
    'src/main.rs',
)

cargo_helper = find_program(join_paths(meson.source_root(), 'build-aux/cargo.sh'))
custom_target(
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

install_data('data/net.gourlaysama.GPowerTweaks.desktop',
    install_dir: join_paths(datadir, 'applications')
)

summary({
    'cargo': run_command(cargo, '-V').stdout().strip(),
    'rustc': run_command(rustc, '-V').stdout().strip(),
    'RUSTFLAGS': rust_flags,
    'debug': get_option('debug')
}, section: 'Rust')