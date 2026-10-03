set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

builddir := "build"
prefix := builddir + "/dev-prefix"

default:
    @just --list

setup:
    meson setup {{ builddir }} --prefix "$PWD/{{ prefix }}"

build: setup
    meson compile -C {{ builddir }}

install: build
    meson install -C {{ builddir }}

run *ARGS: install
    cd {{ builddir }} && \
        env \
            GSETTINGS_SCHEMA_DIR="$PWD/dev-prefix/share/glib-2.0/schemas" \
            XDG_DATA_DIRS="$PWD/dev-prefix/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}" \
            ./src/tsukimi {{ ARGS }}

preview *ARGS: install
    cd {{ builddir }} && \
        env \
            GSETTINGS_SCHEMA_DIR="$PWD/dev-prefix/share/glib-2.0/schemas" \
            XDG_DATA_DIRS="$PWD/dev-prefix/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}" \
            ./src/tsukimi --ui-preview {{ ARGS }}

preview-inspector *ARGS: install
    cd {{ builddir }} && \
        env \
            GTK_DEBUG=interactive \
            GSETTINGS_SCHEMA_DIR="$PWD/dev-prefix/share/glib-2.0/schemas" \
            XDG_DATA_DIRS="$PWD/dev-prefix/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}" \
            ./src/tsukimi --ui-preview {{ ARGS }}

test *ARGS:
    mkdir -p target/test-schemas
    glib-compile-schemas --strict --targetdir target/test-schemas resources
    env \
        GSETTINGS_BACKEND=memory \
        GSETTINGS_SCHEMA_DIR="$PWD/target/test-schemas" \
        cargo test {{ ARGS }}

# Native desktop required; uses fixture accounts, images and a loopback server.
ui-audit theme="3":
    mkdir -p target/ui-audit
    glib-compile-resources resources/resources.gresource.xml --sourcedir=resources --target=target/ui-audit/tsukimi.gresource
    glib-compile-schemas --strict --targetdir target/ui-audit resources
    env TSUKIMI_PKGDATADIR="$PWD/target/ui-audit" cargo build --locked --bin tsukimi --example ui_audit
    env \
        GSETTINGS_BACKEND=memory \
        GSETTINGS_SCHEMA_DIR="$PWD/target/ui-audit" \
        TSUKIMI_UI_AUDIT_DIR="$PWD/target/ui-audit" \
        DYLD_FALLBACK_LIBRARY_PATH="${DYLD_FALLBACK_LIBRARY_PATH:-$(pkg-config --variable=libdir epoxy)}" \
        AUDIT_THEME="{{ theme }}" \
        ./target/debug/examples/ui_audit --ui-preview --log-level warn

update-i18n:
    meson compile -C {{ builddir }} tsukimi-pot
