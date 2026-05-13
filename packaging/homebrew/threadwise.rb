class Threadwise < Formula
  desc "Local session advisor for CLI coding agents"
  homepage "https://github.com/amir-h-rassafi/threadwise"
  version "0.1.2"
  license "MIT OR Apache-2.0"

  on_macos do
    on_arm do
      url "https://github.com/amir-h-rassafi/threadwise/releases/download/v#{version}/threadwise-darwin-arm64.tar.gz"
      sha256 "REPLACE_WITH_DARWIN_ARM64_SHA256"
    end
    on_intel do
      url "https://github.com/amir-h-rassafi/threadwise/releases/download/v#{version}/threadwise-darwin-amd64.tar.gz"
      sha256 "REPLACE_WITH_DARWIN_AMD64_SHA256"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/amir-h-rassafi/threadwise/releases/download/v#{version}/threadwise-linux-arm64.tar.gz"
      sha256 "REPLACE_WITH_LINUX_ARM64_SHA256"
    end
    on_intel do
      url "https://github.com/amir-h-rassafi/threadwise/releases/download/v#{version}/threadwise-linux-amd64.tar.gz"
      sha256 "REPLACE_WITH_LINUX_AMD64_SHA256"
    end
  end

  def install
    bin.install "tw"
  end

  test do
    assert_match "tw #{version}", shell_output("#{bin}/tw --version")
  end
end
