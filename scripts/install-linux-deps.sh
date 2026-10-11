#!/usr/bin/env bash
set -euo pipefail

# Debian/Ubuntu build libraries plus a software-rendered display for PTY smoke
# tests. CI and the local Docker mirror use the same prerequisites.
apt-get update
apt-get install -y --no-install-recommends \
  build-essential clang cmake pkg-config libssl-dev libfontconfig1-dev \
    libfreetype6-dev libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev \
    libx11-dev libx11-xcb-dev libxcb1-dev libxi-dev libxrandr-dev libxcursor-dev \
    libdbus-1-dev libegl1-mesa-dev libvulkan-dev mesa-vulkan-drivers \
    libgtk-3-dev libwebkit2gtk-4.1-dev \
    dbus-x11 xvfb xauth python3 fonts-dejavu-core openssh-client openssh-server
# Required by sshd privilege separation in the isolated loopback test fixture.
install -d -m 0755 /run/sshd
