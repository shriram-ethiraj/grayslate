(function () {
  const root = document.documentElement;
  const themeMeta = document.querySelector('meta[name="theme-color"]');
  const darkQuery = window.matchMedia("(prefers-color-scheme: dark)");

  function readStoredTheme() {
    try {
      const stored = window.localStorage.getItem("grayslate-theme");
      return stored === "light" || stored === "dark" ? stored : null;
    } catch {
      return null;
    }
  }

  function applyTheme(theme) {
    root.dataset.theme = theme;
    themeMeta?.setAttribute("content", theme === "dark" ? "#1b1e26" : "#f0f2f7");
  }

  applyTheme(readStoredTheme() ?? (darkQuery.matches ? "dark" : "light"));
})();
