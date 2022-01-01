#!/bin/sh

pacman -Sy
pacman -Sy --noconfirm rustup
rustup toolchain install stable
