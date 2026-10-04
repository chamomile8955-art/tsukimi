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

# Native desktop and ffmpeg required. Uses generated silent videos only.
local-player-audit:
    mkdir -p target/local-player-audit/videos/child target/ui-audit
    glib-compile-resources resources/resources.gresource.xml --sourcedir=resources --target=target/ui-audit/tsukimi.gresource
    glib-compile-schemas --strict --targetdir target/ui-audit resources
    ffmpeg -hide_banner -loglevel error -y -f lavfi -i testsrc2=size=640x360:rate=12 -f lavfi -i anullsrc=channel_layout=stereo:sample_rate=48000 -t 30 -c:v libx264 -preset ultrafast -pix_fmt yuv420p -c:a aac 'target/local-player-audit/videos/Episode 2.mp4'
    cp 'target/local-player-audit/videos/Episode 2.mp4' 'target/local-player-audit/videos/Episode 1.mp4'
    cp 'target/local-player-audit/videos/Episode 2.mp4' 'target/local-player-audit/videos/Episode 10.MP4'
    cp 'target/local-player-audit/videos/Episode 2.mp4' 'target/local-player-audit/videos/电影 空格.mp4'
    cp LICENSE target/local-player-audit/videos/notes.txt
    cp 'target/local-player-audit/videos/Episode 2.mp4' target/local-player-audit/videos/child/hidden.mp4
    env TSUKIMI_PKGDATADIR="$PWD/target/ui-audit" cargo build --locked --bin tsukimi --example local_player_audit
    env \
        GSETTINGS_SCHEMA_DIR="$PWD/target/ui-audit" \
        TSUKIMI_LOCAL_AUDIT_DIR="$PWD/target/local-player-audit" \
        DYLD_FALLBACK_LIBRARY_PATH="${DYLD_FALLBACK_LIBRARY_PATH:-$(pkg-config --variable=libdir epoxy)}" \
        ./target/debug/examples/local_player_audit --log-file "$PWD/target/local-player-audit/forbidden.log" -- "$PWD/target/local-player-audit/videos/Episode 2.mp4"
    env \
        GSETTINGS_SCHEMA_DIR="$PWD/target/ui-audit" \
        TSUKIMI_LOCAL_AUDIT_DIR="$PWD/target/local-player-audit" \
        LOCAL_AUDIT_EMPTY=1 \
        DYLD_FALLBACK_LIBRARY_PATH="${DYLD_FALLBACK_LIBRARY_PATH:-$(pkg-config --variable=libdir epoxy)}" \
        ./target/debug/examples/local_player_audit --local-player --log-file "$PWD/target/local-player-audit/forbidden.log" -- "$PWD/target/local-player-audit/videos/notes.txt"

update-i18n:
    meson compile -C {{ builddir }} tsukimi-pot
