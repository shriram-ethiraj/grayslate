export type SupportedOS = "macos" | "windows" | "linux";

export interface DownloadAlternative {
  format: string;
  subtitle: string;
  href: string;
}

export interface DownloadTarget {
  os: SupportedOS;
  label: string;
  shortLabel: string;
  format: string;
  downloadSubtitle: string;
  architecture: string;
  href: string;
  fallbackHref: string;
  installCommand?: string;
  installLabel?: string;
  installPrimary?: boolean;
  installAnchor?: string;
  alternatives?: readonly DownloadAlternative[];
}

const REPOSITORY_URL = "https://github.com/shriram-ethiraj/grayslate";
const RELEASES_URL = `${REPOSITORY_URL}/releases`;
const LATEST_RELEASE_URL = `${RELEASES_URL}/latest`;
const PACKAGES_URL = "https://packages.grayslate.app";

export const site = {
  name: "Grayslate",
  title: "Grayslate — A fast scratchpad for code and data",
  description:
    "A local developer scratchpad that recognizes pasted content, suggests relevant transformations, and automatically names and saves new slates so they are easy to find later.",
  url: "https://grayslate.app",
  repositoryUrl: REPOSITORY_URL,
  issuesUrl: `${REPOSITORY_URL}/issues`,
  licenseUrl: `${REPOSITORY_URL}/blob/main/LICENSE`,
  releasesUrl: RELEASES_URL,
  latestReleaseUrl: LATEST_RELEASE_URL,
  packagesUrl: PACKAGES_URL,
} as const;

export const downloads: readonly DownloadTarget[] = [
  {
    os: "macos",
    label: "Download for macOS",
    shortLabel: "macOS",
    format: "DMG",
    downloadSubtitle: "For Apple Silicon and Intel Macs",
    architecture: "Apple Silicon + Intel",
    href: `${LATEST_RELEASE_URL}/download/grayslate-macos-universal.dmg`,
    fallbackHref: LATEST_RELEASE_URL,
    installCommand: "brew install --cask shriram-ethiraj/grayslate/grayslate",
    installLabel: "Install with Homebrew",
    installAnchor: "#download-macos",
  },
  {
    os: "windows",
    label: "Download for Windows",
    shortLabel: "Windows",
    format: "x64",
    downloadSubtitle: "For most Windows PCs",
    architecture: "x64 + ARM64",
    href: `${LATEST_RELEASE_URL}/download/grayslate-windows-x86_64-setup.exe`,
    fallbackHref: LATEST_RELEASE_URL,
    alternatives: [
      {
        format: "ARM64",
        subtitle: "Windows on ARM",
        href: `${LATEST_RELEASE_URL}/download/grayslate-windows-aarch64-setup.exe`,
      },
    ],
  },
  {
    os: "linux",
    label: "Download for Linux",
    shortLabel: "Linux",
    format: "AppImage",
    downloadSubtitle: "Standalone",
    architecture: "x86_64",
    href: `${LATEST_RELEASE_URL}/download/grayslate-linux-x86_64.AppImage`,
    fallbackHref: LATEST_RELEASE_URL,
    installCommand: `curl -fsSL ${PACKAGES_URL}/install.sh | sh`,
    installLabel: "Install on Debian / Fedora",
    installPrimary: true,
    installAnchor: "#download-linux",
  },
] as const;

export const proofPoints = [
  { value: "80+", label: "local transformations" },
  { value: "40+", label: "languages recognized" },
  { value: "200 MB", label: "maximum file size" },
  { value: "Auto-saved", label: "named and searchable" },
] as const;
