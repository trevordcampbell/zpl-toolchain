# Use Debian Trixie as the base image
FROM debian:trixie

# Set environment variables (optional, but good practice)
# ENV DEBIAN_FRONTEND=noninteractive
ENV LANG=C.UTF-8

# Update package lists and install any necessary packages
RUN apt-get update && apt-get install -y --no-install-recommends \
    # specific packages here
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspaces/zpl-toolchain