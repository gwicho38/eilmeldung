class Dispatch < Formula
  desc "a feature-rich TUI RSS reader based on the newsflash library"
  homepage "https://github.com/christo-auer/dispatch"
  url "https://github.com/christo-auer/dispatch/archive/refs/tags/1.1.0.tar.gz"
  sha256 "566aa7ec5477cd66ecf88a30f3faaeb84f873628dd11e5724f54a89677298b3c"
  license "GPL-3.0"
  head "https://github.com/christo-auer/dispatch.git", branch: "main"
  version "1.1.0"

  depends_on "pkg-config" => :build
  depends_on "rust" => :build

  depends_on "libxml2"
  depends_on "openssl@3"
  depends_on "sqlite"

  on_linux do
    depends_on "llvm" => :build
  end
  
  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "dispatch #{version}", shell_output("#{bin}/dispatch --version").strip
  end


end
