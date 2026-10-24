name := 'clover-applet'
appid := 'io.github.onuraksoy3.Clover'
rootdir := ''
prefix := '/usr'

appdata := appid + '.metainfo.xml'
desktop := appid + '.desktop'

# Installation paths
base-dir := absolute_path(clean(rootdir / prefix))
cargo-target-dir := env('CARGO_TARGET_DIR', 'target')
appdata-dst := base-dir / 'share' / 'appdata' / appdata
bin-dst := base-dir / 'bin' / name
desktop-dst := base-dir / 'share' / 'applications' / desktop
icon-dst := base-dir / 'share' / 'icons' / 'hicolor' / 'scalable' / 'apps' / appid + '.svg'

# Default recipe which runs `just build-release`
default: build-release

# Runs `cargo clean`
clean:
    cargo clean

# Removes vendored dependencies
clean-vendor:
    rm -rf .cargo vendor vendor.tar

# `cargo clean` and removes vendored dependencies
clean-dist: clean clean-vendor

# Compiles with debug profile
build-debug *args:
    cargo build --workspace {{args}}

# Compiles with release profile
build-release *args: (build-debug '--release' args)

# Compiles release profile with vendored dependencies
build-vendored *args: vendor-extract (build-release '--frozen --offline' args)

# Runs a clippy check
check *args:
    cargo clippy --all-features {{args}} -- -W clippy::pedantic

# Runs a clippy check with JSON message format
check-json: (check '--message-format=json')

# Run the application for testing purposes
run *args:
    env RUST_BACKTRACE=full cargo run --release {{args}}

# Installs files
install:
    install -Dm0755 {{ cargo-target-dir / 'release' / name }} {{bin-dst}}
    install -Dm0644 {{ 'target' / 'xdgen' / 'app.desktop' }} {{desktop-dst}}
    install -Dm0644 {{ 'target' / 'xdgen' / 'app.metainfo.xml' }} {{appdata-dst}}
    install -Dm0644 resources/icon.svg {{icon-dst}}
    install -Dm0755 scripts/clover {{base-dir}}/bin/clover
    install -Dm0755 scripts/clover-empty-trash {{base-dir}}/bin/clover-empty-trash
    install -Dm0755 scripts/clover-kbd-backlight {{base-dir}}/bin/clover-kbd-backlight
    install -Dm0755 scripts/clover-screenshot {{base-dir}}/bin/clover-screenshot
    install -Dm0755 scripts/clover-sensord {{base-dir}}/bin/clover-sensord
    install -Dm0755 scripts/clover-gpu-rules {{base-dir}}/bin/clover-gpu-rules
    install -Dm0755 scripts/clover-fnmode {{base-dir}}/share/clover/clover-fnmode
    install -Dm0644 files/clover-sensord.service {{base-dir}}/share/clover/clover-sensord.service
    install -Dm0755 {{cargo-target-dir}}/release/clover-corners {{base-dir}}/bin/clover-corners
    install -Dm0644 files/clover-corners.service {{base-dir}}/share/clover/clover-corners.service
    install -Dm0755 scripts/clover-fix {{base-dir}}/share/clover/clover-fix
    install -Dm0644 examples/keyd-default.conf {{base-dir}}/share/clover/keyd-default.conf

# Uninstalls installed files
uninstall:
    rm -f {{bin-dst}} {{desktop-dst}} {{icon-dst}} {{base-dir}}/bin/clover {{base-dir}}/bin/clover-empty-trash {{base-dir}}/bin/clover-kbd-backlight {{base-dir}}/bin/clover-screenshot {{base-dir}}/bin/clover-sensord {{base-dir}}/bin/clover-gpu-rules {{base-dir}}/bin/clover-corners
    rm -rf {{base-dir}}/share/clover

# Vendor dependencies locally
vendor:
    mkdir -p .cargo
    cargo vendor --sync Cargo.toml | head -n -1 > .cargo/config.toml
    echo 'directory = "vendor"' >> .cargo/config.toml
    echo >> .cargo/config.toml
    rm -rf .cargo vendor

# Extracts vendored dependencies
vendor-extract:
    rm -rf vendor
    tar pxf vendor.tar

# Bump cargo version, create git commit, and create tag
tag version:
    find -type f -name Cargo.toml -exec sed -i '0,/^version/s/^version.*/version = "{{version}}"/' '{}' \; -exec git add '{}' \;
    cargo check
    cargo clean
    git add Cargo.lock
    git commit -m 'release: {{version}}'
    git tag -a {{version}} -m ''
