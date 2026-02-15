# Use Debian Trixie as the base image
FROM debian:trixie

# Set environment variables (optional, but good practice)
# ENV DEBIAN_FRONTEND=noninteractive
ENV LANG=C.UTF-8

# Update package lists and install runtime dependencies used by
# VS Code Extension Host integration tests (via @vscode/test-electron).
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    git \
    libasound2t64 \
    libatk-bridge2.0-0 \
    libgbm1 \
    libglib2.0-0 \
    libgtk-3-0 \
    libnss3 \
    libx11-6 \
    libxcomposite1 \
    libxdamage1 \
    libxfixes3 \
    libxkbfile1 \
    libxrandr2 \
    libxss1 \
    xvfb \
    xauth \
    unzip \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspaces/zpl-toolchain