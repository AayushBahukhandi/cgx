class Cgx < Formula
  desc "Turn any Git repository into a queryable knowledge graph"
  homepage "https://github.com/AayushBahukhandi/cgx"
  url "https://github.com/AayushBahukhandi/cgx/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "YOUR_TARBALL_SHA256"
  license "MIT"

  # Pre-built binaries — users get these by default
  if OS.mac? && Hardware::CPU.intel?
    url "https://github.com/AayushBahukhandi/cgx/releases/download/v0.1.0/cgx-v0.1.0-x86_64-apple-darwin.tar.gz"
    sha256 "YOUR_DARWIN_INTEL_SHA256"
  elsif OS.mac? && Hardware::CPU.arm?
    url "https://github.com/AayushBahukhandi/cgx/releases/download/v0.1.0/cgx-v0.1.0-aarch64-apple-darwin.tar.gz"
    sha256 "YOUR_DARWIN_ARM_SHA256"
  elsif OS.linux? && Hardware::CPU.intel?
    url "https://github.com/AayushBahukhandi/cgx/releases/download/v0.1.0/cgx-v0.1.0-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "YOUR_LINUX_INTEL_SHA256"
  end

  def install
    bin.install "cgx"
    pkgshare.install "web-ui"
  end

  def caveats
    <<~EOS
      cgx has been installed.

      Quick start:
        cd your-project
        cgx analyze
        cgx view --web

      For AI editor integration:
        cgx setup

      To check your installation:
        cgx doctor
    EOS
  end

  test do
    system "#{bin}/cgx", "--version"
  end
end
