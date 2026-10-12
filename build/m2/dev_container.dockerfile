# M2 disposable Debian 13 qualification/development container.
#
# Build (context is build/m2):
#   docker build -f build/m2/dev_container.dockerfile -t cham3-m2-dev build/m2
# The base image is pinned by its multi-architecture index digest; the same file
# serves the amd64 qualification target and native arm64 development hosts. Only
# amd64 results qualify the M2 target (PROJECT.md: x86-64 primary architecture).
#
# Status: PROPOSED G1 environment (docs/decisions/0003_m2_rust_toolchain_dependencies.md).
# This image is disposable. It contains no keys, credentials or host configuration.
FROM debian@sha256:913f6706df59a68922d1dd08f78c2476560a8d367897200a6005b00e5f67c2d5

ENV DEBIAN_FRONTEND=noninteractive \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8

COPY debian_packages.txt /usr/local/share/cham3/m2/debian_packages.txt

# Install exactly the pinned package versions. A missing or superseded version is a
# build failure, never a silent upgrade. APT authenticates the Release/Packages
# metadata with the Debian archive keys shipped in the base image, and the SHA-256 of
# every downloaded .deb is recorded for build/scripts/qualify_m2_rust.sh. Packages are
# hashed after a download-only pass because the base image's docker-clean hook deletes
# .deb files once dpkg runs.
RUN set -eu; \
    apt-get update; \
    grep -v -e '^#' -e '^$' /usr/local/share/cham3/m2/debian_packages.txt > /tmp/pins.txt; \
    xargs -a /tmp/pins.txt apt-get install -y --no-install-recommends --download-only; \
    cd /var/cache/apt/archives; \
    sha256sum -- *.deb > /usr/local/share/cham3/m2/downloaded_debs.sha256; \
    cd /; \
    xargs -a /tmp/pins.txt apt-get install -y --no-install-recommends; \
    rm -f /tmp/pins.txt; \
    apt-get clean; \
    rm -rf /var/lib/apt/lists/*

# Unprivileged builder. Qualification and builds never need root.
RUN useradd --create-home --uid 1000 --shell /bin/bash builder

USER builder
WORKDIR /home/builder
