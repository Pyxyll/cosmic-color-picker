name        := 'cosmic-color-picker'
prefix      := '/usr'
bin-dir     := prefix / 'bin'
app-dir     := prefix / 'share' / 'applications'
unit-dir    := prefix / 'lib' / 'systemd' / 'user'
icon-dir    := prefix / 'share' / 'icons' / 'hicolor' / 'scalable' / 'apps'
metainfo-dir:= prefix / 'share' / 'metainfo'
i18n-dir    := prefix / 'share' / name

# Default: build all three release binaries.
default: build-release

build-release:
    cargo build --release --workspace

# Build, then install everything to {{prefix}} (use sudo).
install: build-release
    install -Dm0755 target/release/cosmic-color-pickerd       {{bin-dir}}/cosmic-color-pickerd
    install -Dm0755 target/release/cosmic-color-picker        {{bin-dir}}/cosmic-color-picker
    install -Dm0755 target/release/cosmic-applet-color-picker {{bin-dir}}/cosmic-applet-color-picker
    install -Dm0644 gui/resources/com.pyxyll.CosmicColorPicker.desktop \
                    {{app-dir}}/com.pyxyll.CosmicColorPicker.desktop
    install -Dm0644 applet/resources/com.pyxyll.CosmicColorPickerApplet.desktop \
                    {{app-dir}}/com.pyxyll.CosmicColorPickerApplet.desktop
    install -Dm0644 gui/resources/com.pyxyll.CosmicColorPicker.svg \
                    {{icon-dir}}/com.pyxyll.CosmicColorPicker.svg
    install -Dm0644 gui/resources/com.pyxyll.CosmicColorPicker.metainfo.xml \
                    {{metainfo-dir}}/com.pyxyll.CosmicColorPicker.metainfo.xml
    install -Dm0644 dist/systemd/cosmic-color-pickerd.service \
                    {{unit-dir}}/cosmic-color-pickerd.service

uninstall:
    rm -f {{bin-dir}}/cosmic-color-pickerd
    rm -f {{bin-dir}}/cosmic-color-picker
    rm -f {{bin-dir}}/cosmic-applet-color-picker
    rm -f {{app-dir}}/com.pyxyll.CosmicColorPicker.desktop
    rm -f {{app-dir}}/com.pyxyll.CosmicColorPickerApplet.desktop
    rm -f {{icon-dir}}/com.pyxyll.CosmicColorPicker.svg
    rm -f {{metainfo-dir}}/com.pyxyll.CosmicColorPicker.metainfo.xml
    rm -f {{unit-dir}}/cosmic-color-pickerd.service

clean:
    cargo clean
