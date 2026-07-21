// Three small behaviours, no framework. Everything the site does without
// JavaScript it still does with it turned off; this is polish, not plumbing.
(function () {
  "use strict";

  // --- Theme -----------------------------------------------------------------
  // The boot script in `Base` has already applied the stored choice before the
  // first paint. This only handles changing it.
  const STORAGE_KEY = "damask-theme";

  document.addEventListener("click", (event) => {
    const toggle = event.target.closest("[data-theme-toggle]");
    if (!toggle) return;

    const root = document.documentElement;
    const next = root.dataset.theme === "dark" ? "light" : "dark";
    root.dataset.theme = next;
    try {
      localStorage.setItem(STORAGE_KEY, next);
    } catch (e) {
      // Private mode, or storage disabled. The theme still applies to this
      // page; it just will not survive a navigation, which is a better outcome
      // than a click that appears to do nothing.
    }
  });

  // --- Copy buttons ----------------------------------------------------------
  // Added here rather than in the markup because a code block is one opaque
  // HTML string by the time a template sees it. `pre` scrolls, so the button
  // goes on a wrapper — inside, it would scroll away with the code.
  function addCopyButtons() {
    if (!navigator.clipboard) return;

    document.querySelectorAll("pre.code").forEach((pre) => {
      if (pre.parentElement.classList.contains("code-block")) return;

      const wrapper = document.createElement("div");
      wrapper.className = "code-block";
      pre.replaceWith(wrapper);
      wrapper.append(pre);

      const button = document.createElement("button");
      button.type = "button";
      button.className = "code-copy";
      button.textContent = "Copy";
      button.setAttribute("aria-label", "Copy code to clipboard");

      button.addEventListener("click", async () => {
        try {
          await navigator.clipboard.writeText(pre.textContent);
          button.textContent = "Copied";
          button.dataset.copied = "";
          setTimeout(() => {
            button.textContent = "Copy";
            delete button.dataset.copied;
          }, 1600);
        } catch (e) {
          button.textContent = "Failed";
          setTimeout(() => (button.textContent = "Copy"), 1600);
        }
      });

      wrapper.append(button);
    });
  }

  // --- Contents ---------------------------------------------------------------
  // Marks the entry for the heading currently in view.
  //
  // The root margin pins the "current" line a fifth of the way down the
  // viewport: keying off whatever is at the very top makes the highlight jump a
  // section early on every scroll, and keying off the middle leaves the first
  // heading unmarked until it has scrolled halfway up.
  function trackHeadings() {
    const links = document.querySelectorAll("[data-toc-link]");
    if (!links.length) return;

    const byId = new Map();
    links.forEach((link) => byId.set(link.dataset.tocLink, link));

    const headings = [...byId.keys()]
      .map((id) => document.getElementById(id))
      .filter(Boolean);
    if (!headings.length) return;

    let visible = new Set();

    const mark = () => {
      // The topmost visible heading wins, so a section whose whole body is on
      // screen does not lose to the next one appearing at the bottom.
      const current = headings.find((heading) => visible.has(heading.id));
      links.forEach((link) => delete link.dataset.active);
      if (current) byId.get(current.id).dataset.active = "";
    };

    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) visible.add(entry.target.id);
          else visible.delete(entry.target.id);
        });
        mark();
      },
      { rootMargin: "-20% 0px -70% 0px" }
    );

    headings.forEach((heading) => observer.observe(heading));
  }

  addCopyButtons();
  trackHeadings();
})();
