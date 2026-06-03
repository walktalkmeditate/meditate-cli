class Meditate < Formula
  desc "Terminal breathing companion — paced breathing, soundscapes, and voice guides"
  homepage "https://github.com/walktalkmeditate/meditate-cli"
  version "${VERSION}"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/walktalkmeditate/meditate-cli/releases/download/${TAG}/meditate-aarch64-apple-darwin.tar.gz"
      sha256 "${SHA256_MAC_ARM}"
    end
    on_intel do
      url "https://github.com/walktalkmeditate/meditate-cli/releases/download/${TAG}/meditate-x86_64-apple-darwin.tar.gz"
      sha256 "${SHA256_MAC_X86}"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/walktalkmeditate/meditate-cli/releases/download/${TAG}/meditate-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "${SHA256_LINUX_X86}"
    end
  end

  def install
    bin.install "meditate"
  end

  test do
    assert_match "meditate", shell_output("#{bin}/meditate --version")
  end
end
