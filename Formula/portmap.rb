class Portmap < Formula
  desc "Map names to localhost ports. Made for agents and humans."
  homepage "https://github.com/jonasKs/portmap"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/jonasKs/portmap/releases/download/v#{version}/portmap-aarch64-apple-darwin.tar.gz"
      # sha256 "UPDATE_ON_RELEASE"
    end

    on_intel do
      url "https://github.com/jonasKs/portmap/releases/download/v#{version}/portmap-x86_64-apple-darwin.tar.gz"
      # sha256 "UPDATE_ON_RELEASE"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/jonasKs/portmap/releases/download/v#{version}/portmap-aarch64-unknown-linux-gnu.tar.gz"
      # sha256 "UPDATE_ON_RELEASE"
    end

    on_intel do
      url "https://github.com/jonasKs/portmap/releases/download/v#{version}/portmap-x86_64-unknown-linux-gnu.tar.gz"
      # sha256 "UPDATE_ON_RELEASE"
    end
  end

  def install
    bin.install "portmap"
  end

  service do
    run [opt_bin/"portmap", "--port", "1337"]
    keep_alive true
    log_path var/"log/portmap.log"
    error_log_path var/"log/portmap.log"
  end

  test do
    assert_match "portmap", shell_output("#{bin}/portmap --help")
  end
end
