# Build image for the Linux app.
#
# 24.04 on purpose: this container is the compatibility floor the AppImage is
# built against (glibc 2.39, GTK 4.14, libadwaita 1.5). Building against newer
# libraries would link against symbols this floor does not have, so the build
# always happens in this image and the gtk4/libadwaita version features in
# Cargo.toml are pinned to match it.
#
# Builds only. The tray, notifications and portals need a real session bus and
# a compositor, so the binary has to be run on a desktop session.
FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive

# libgtk-4-dev/libadwaita-1-dev: the UI. libdbus-1-dev + protobuf-compiler:
# rqs_lib's build dependencies, same as the CI job installs.
# adwaita-icon-theme and hicolor-icon-theme are NOT optional. GTK's own
# dependencies do not pull them, and without them linuxdeploy has no icon theme
# to bundle: every named icon fails to render, and the missing
# hicolor/index.theme even prevents GTK from finding the app's own icon, so the
# window falls back to a generic one.
RUN apt-get update && apt-get install -y --no-install-recommends \
        adwaita-icon-theme \
        build-essential \
        ca-certificates \
        curl \
        git \
        hicolor-icon-theme \
        libadwaita-1-dev \
        libdbus-1-dev \
        libgtk-4-dev \
        pkg-config \
        protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# AppImage tooling. `file` and `desktop-file-utils` are what linuxdeploy shells
# out to; `librsvg2-common` provides the gdk-pixbuf SVG loader WITHOUT WHICH
# every SVG-only icon renders as a broken image — `librsvg2-bin` is only the
# rsvg-convert command line tool and is not a substitute; `patchelf`
# rewrites the rpaths of everything bundled; `zsync` writes the delta files
# release updates use.
RUN apt-get update && apt-get install -y --no-install-recommends \
        desktop-file-utils \
        file \
        librsvg2-bin \
        librsvg2-common \
        patchelf \
        squashfs-tools \
        zsync \
    && rm -rf /var/lib/apt/lists/*

# linuxdeploy and its GTK plugin, pinned to the architecture of the image.
# Fetched at build time so the packaging step needs no network of its own.
RUN set -eux; \
    arch="$(uname -m)"; \
    base=https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous; \
    curl -fsSL -o /usr/local/bin/linuxdeploy "$base/linuxdeploy-$arch.AppImage"; \
    curl -fsSL -o /usr/local/bin/linuxdeploy-plugin-gtk \
        https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gtk/master/linuxdeploy-plugin-gtk.sh; \
    curl -fsSL -o /usr/local/bin/appimagetool \
        "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-$arch.AppImage"; \
    chmod +x /usr/local/bin/linuxdeploy /usr/local/bin/linuxdeploy-plugin-gtk /usr/local/bin/appimagetool

# Containers have no FUSE, so the AppImage tools must unpack themselves instead
# of mounting. This is the documented escape hatch, not a workaround.
ENV APPIMAGE_EXTRACT_AND_RUN=1

# rustup honours the repo's rust-toolchain.toml, so the channel is not pinned here.
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --no-modify-path --profile minimal --component clippy,rustfmt
ENV PATH=/root/.cargo/bin:$PATH

WORKDIR /src/apps/linux
