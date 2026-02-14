# typed: false
# frozen_string_literal: true

# Homebrew formula for zpl-toolchain CLI.
# Install from in-repo: brew install --formula Formula/zpl-toolchain.rb
# Or add a tap and: brew install trevordcampbell/zpl-toolchain/zpl-toolchain
#
# Platform support (aligned with packages/ts/cli/lib/platform.mjs):
#   - Linux x86_64
#   - macOS ARM64 (Apple Silicon)
#
# Unsupported: macOS Intel, Linux ARM64.
class ZplToolchain < Formula
  desc "ZPL II toolchain for parsing, validating, formatting, and printing Zebra labels"
  homepage "https://github.com/trevordcampbell/zpl-toolchain"
  version "0.1.12"
  license "MIT OR Apache-2.0"

  on_macos do
    on_arm do
      url "https://github.com/trevordcampbell/zpl-toolchain/releases/download/v0.1.12/zpl-aarch64-apple-darwin.tar.gz"
      sha256 "0cb6637fb6a65599f7c8d2eae862215449ab45cd0d9abadb1520956c26c0674e"
    end
    on_intel do
      # No pre-built binary for Intel Mac
      odie "zpl-toolchain does not provide a pre-built binary for Intel Mac. Use: cargo install zpl_toolchain_cli"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/trevordcampbell/zpl-toolchain/releases/download/v0.1.12/zpl-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "f7318711adc7e878074847d01a1f8394c89b73966e750841e30d66670247c570"
    end
    on_arm do
      odie "zpl-toolchain does not provide a pre-built binary for Linux ARM64. Use: cargo install zpl_toolchain_cli"
    end
  end

  def install
    bin.install "zpl"
  end

  test do
    assert_match(/zpl \d+\.\d+/, shell_output("#{bin}/zpl --version 2>&1"))
  end
end
