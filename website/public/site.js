const root = document.documentElement;
const themeMeta = document.querySelector('meta[name="theme-color"]');
const darkQuery = window.matchMedia("(prefers-color-scheme: dark)");

function getStoredTheme() {
  try {
    const stored = window.localStorage.getItem("grayslate-theme");
    return stored === "light" || stored === "dark" ? stored : null;
  } catch {
    return null;
  }
}

function setTheme(theme, persist) {
  root.dataset.theme = theme;
  themeMeta?.setAttribute("content", theme === "dark" ? "#1b1e26" : "#f0f2f7");

  document.querySelectorAll("[data-theme-picture]").forEach((picture) => {
    const lightSource = picture.querySelector('[data-theme-source="light"]');
    if (lightSource instanceof HTMLSourceElement) {
      lightSource.media = theme === "light" ? "all" : "not all";
    }
  });

  const toggle = document.querySelector("[data-theme-toggle]");
  toggle?.setAttribute("aria-label", theme === "dark" ? "Use light theme" : "Use dark theme");

  if (!persist) return;
  try {
    window.localStorage.setItem("grayslate-theme", theme);
  } catch {
    // A private browsing policy may deny storage; the active theme still works.
  }
}

document.querySelector("[data-theme-toggle]")?.addEventListener("click", () => {
  setTheme(root.dataset.theme === "dark" ? "light" : "dark", true);
});

darkQuery.addEventListener("change", (event) => {
  if (!getStoredTheme()) setTheme(event.matches ? "dark" : "light", false);
});

setTheme(root.dataset.theme === "light" ? "light" : "dark", false);

const header = document.querySelector("[data-site-header]");
function updateHeader() {
  if (header instanceof HTMLElement) header.dataset.scrolled = String(window.scrollY > 12);
}
updateHeader();
window.addEventListener("scroll", updateHeader, { passive: true });

function detectOS() {
  const userAgentData = navigator.userAgentData;
  const platform = `${userAgentData?.platform ?? navigator.platform ?? ""} ${navigator.userAgent}`.toLowerCase();
  if (platform.includes("mac")) return "macos";
  if (platform.includes("win")) return "windows";
  if (platform.includes("linux") || platform.includes("x11")) return "linux";
  return null;
}

const detectedOS = detectOS();
if (detectedOS) {
  const source = document.querySelector(`[data-download-source][data-os="${detectedOS}"]`);
  const primaryDownload = document.querySelector("[data-primary-download]");
  const label = source?.getAttribute("data-label");
  const href = source?.getAttribute("data-href");
  const installAnchor = source?.getAttribute("data-install-anchor");

  if (primaryDownload instanceof HTMLAnchorElement && label && href) {
    primaryDownload.href = installAnchor || href;
    const labelElement = primaryDownload.querySelector("[data-download-label]");
    if (labelElement) {
      labelElement.textContent = installAnchor ? label.replace("Download", "Install") : label;
    }
  }

  document.querySelector(`[data-platform-download="${detectedOS}"]`)?.setAttribute("data-recommended", "true");
}

document.querySelectorAll("[data-copy-command]").forEach((button) => {
  button.addEventListener("click", async () => {
    const container = button.closest(".install-method");
    const code = container?.querySelector("code");
    const label = button.querySelector("[data-copy-label]");
    const status = container?.querySelector("[data-copy-status]");
    const command = code?.textContent?.trim();
    if (!command) return;

    try {
      await navigator.clipboard.writeText(command);
      if (label) label.textContent = "Copied";
      if (status) status.textContent = "Command copied to clipboard.";
    } catch {
      const selection = window.getSelection();
      const range = document.createRange();
      range.selectNodeContents(code);
      selection?.removeAllRanges();
      selection?.addRange(range);
      if (label) label.textContent = "Selected";
      if (status) status.textContent = "Press Command+C to copy.";
    }

    window.setTimeout(() => {
      if (label) label.textContent = "Copy";
      if (status) status.textContent = "";
    }, 2500);
  });
});

const stage = document.querySelector("[data-product-stage]");
const slides = [...document.querySelectorAll("[data-product-slide]")];
const demoToggle = document.querySelector("[data-demo-toggle]");
const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
let activeSlide = 0;
let demoTimer;

function showSlide(index) {
  slides.forEach((slide, slideIndex) => {
    slide.classList.toggle("product-slide--active", slideIndex === index);
  });
}

function stopDemo() {
  window.clearInterval(demoTimer);
  demoTimer = undefined;
  if (stage instanceof HTMLElement) stage.dataset.paused = "true";
  demoToggle?.setAttribute("aria-label", "Play product preview");
}

function startDemo() {
  if (slides.length < 2 || reduceMotion.matches) {
    stopDemo();
    return;
  }
  window.clearInterval(demoTimer);
  if (stage instanceof HTMLElement) stage.dataset.paused = "false";
  demoToggle?.setAttribute("aria-label", "Pause product preview");
  demoTimer = window.setInterval(() => {
    activeSlide = (activeSlide + 1) % slides.length;
    showSlide(activeSlide);
  }, 4000);
}

demoToggle?.addEventListener("click", () => {
  if (demoTimer) stopDemo();
  else startDemo();
});

reduceMotion.addEventListener("change", (event) => {
  if (event.matches) {
    activeSlide = 0;
    showSlide(activeSlide);
    stopDemo();
  } else {
    startDemo();
  }
});

startDemo();
