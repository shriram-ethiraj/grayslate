export type SupportedOS = "macos" | "windows" | "linux";

export interface DownloadTarget {
  os: SupportedOS;
  label: string;
  shortLabel: string;
  format: string;
  architecture: string;
  href: string;
  fallbackHref: string;
}

const REPOSITORY_URL = "https://github.com/shriram-ethiraj/grayslate";
const RELEASES_URL = `${REPOSITORY_URL}/releases`;
const LATEST_RELEASE_URL = `${RELEASES_URL}/latest`;

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
} as const;

export const downloads: readonly DownloadTarget[] = [
  {
    os: "macos",
    label: "Download for macOS",
    shortLabel: "macOS",
    format: "DMG",
    architecture: "Apple Silicon + Intel",
    href: `${LATEST_RELEASE_URL}/download/grayslate-macos-universal.dmg`,
    fallbackHref: LATEST_RELEASE_URL,
  },
  {
    os: "windows",
    label: "Download for Windows",
    shortLabel: "Windows",
    format: "EXE",
    architecture: "64-bit",
    href: `${LATEST_RELEASE_URL}/download/grayslate-windows-x86_64-setup.exe`,
    fallbackHref: LATEST_RELEASE_URL,
  },
  {
    os: "linux",
    label: "Download for Linux",
    shortLabel: "Linux",
    format: "AppImage",
    architecture: "x86_64",
    href: `${LATEST_RELEASE_URL}/download/grayslate-linux-x86_64.AppImage`,
    fallbackHref: LATEST_RELEASE_URL,
  },
] as const;

export const proofPoints = [
  { value: "80+", label: "local transformations" },
  { value: "40+", label: "languages recognized" },
  { value: "200 MB", label: "maximum file size" },
  { value: "Auto-saved", label: "named and searchable" },
] as const;
