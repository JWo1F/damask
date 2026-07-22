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

  // --- Cloning -----------------------------------------------------------------
  // Every fragment this file creates is authored as markup in `ui/templates.dmk`
  // and cloned here. Nothing below builds HTML from a string.
  function clone(name) {
    const template = document.querySelector(`[data-template="${name}"]`);
    return template ? template.content.firstElementChild.cloneNode(true) : null;
  }

  // --- Copy buttons ----------------------------------------------------------
  // The frame, the rail and the language label are all in the markup already —
  // the generator emits them, so they survive with scripting off. The button is
  // the one part that would do nothing without this file, so it is the one part
  // added here, into the space the rail already reserves for it.
  function addCopyButtons() {
    if (!navigator.clipboard) return;

    document.querySelectorAll("[data-code-rail]").forEach((rail) => {
      // The rail is the block's caption, so the code it labels is its sibling.
      const pre = rail.parentElement.querySelector("pre.code");
      if (!pre || rail.querySelector(".code-copy")) return;

      const button = clone("copy");
      if (!button) return;
      const label = button.querySelector(".code-copy-label");
      let reset;

      // The result is announced as well as shown: the state change is a colour
      // and a glyph, neither of which reaches a reader using a screen reader.
      const settle = (state, text) => {
        button.dataset.state = state;
        label.textContent = text;
        clearTimeout(reset);
        reset = setTimeout(() => {
          delete button.dataset.state;
          label.textContent = "Copy";
        }, 1800);
      };

      button.addEventListener("click", async () => {
        try {
          await navigator.clipboard.writeText(pre.textContent);
          settle("copied", "Copied");
        } catch (e) {
          settle("failed", "Failed");
        }
      });

      rail.append(button);
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

    let current = null;

    const mark = () => {
      // The last heading at or above a line a quarter of the way down the
      // viewport. That heading stays current for the whole of its section,
      // however long it runs.
      //
      // An intersection test cannot do this. It can only report a heading while
      // it is inside a band, so every section taller than the band leaves
      // nothing marked — the highlight blinks on as a heading crosses and off
      // again for the rest of the section.
      const line = window.innerHeight * 0.25;
      let found = headings[0];
      for (const heading of headings) {
        if (heading.getBoundingClientRect().top > line) break;
        found = heading;
      }

      // The last section is often too short to reach the line, so nothing below
      // it could ever become current. At the bottom of the page it is.
      const bottom = window.scrollY + window.innerHeight;
      if (bottom >= document.documentElement.scrollHeight - 2) {
        found = headings[headings.length - 1];
      }

      if (found === current) return;
      current = found;
      links.forEach((link) => delete link.dataset.active);
      byId.get(found.id).dataset.active = "";
    };

    // Reading a bounding box forces layout, so the work is done once per frame
    // rather than once per scroll event.
    let queued = false;
    const onScroll = () => {
      if (queued) return;
      queued = true;
      requestAnimationFrame(() => {
        queued = false;
        mark();
      });
    };

    addEventListener("scroll", onScroll, { passive: true });
    addEventListener("resize", onScroll, { passive: true });
    mark();
  }

  // --- Search -----------------------------------------------------------------
  // Reads `assets/search.idx`, the inverted index the generator writes. See
  // `site/src/search.rs` for the format; the two have to agree byte for byte.
  //
  // Nothing here parses. The response is an ArrayBuffer, every numeric section
  // is a 4-byte-aligned run of u32s, and the views below are windows onto it —
  // so opening the dialog costs a fetch and nothing else. Only the strings a
  // visible result needs are ever decoded.

  const MAGIC = 0x534b4d44; // "DMKS" little-endian
  const K1 = 1.2; // BM25 term-frequency saturation
  const B = 0.75; // BM25 length normalisation
  const MAX_PREFIX_TERMS = 48; // how far one typed prefix may fan out
  const MAX_RESULTS = 8;

  const utf8 = new TextDecoder();
  const utf8Encode = new TextEncoder();

  function readIndex(buffer) {
    const head = new Uint32Array(buffer, 0, 16);
    if (head[0] !== MAGIC || head[1] !== 1) throw new Error("bad search index");

    const docCount = head[2];
    const termCount = head[3];
    const stringCount = head[4];
    const postingCount = head[5];
    const avgLen = head[6] || 1;

    const u32 = (offset, count) => new Uint32Array(buffer, offset, count);
    const bytes = new Uint8Array(buffer);

    const docs = u32(head[7], docCount * 6);
    const termOffsets = u32(head[8], termCount + 1);
    const termBlob = head[9];
    const postingOffsets = u32(head[10], termCount + 1);
    const postings = u32(head[11], postingCount * 2);
    const stringOffsets = u32(head[12], stringCount + 1);
    const stringBlob = head[13];

    const string = (id) =>
      utf8.decode(bytes.subarray(stringBlob + stringOffsets[id], stringBlob + stringOffsets[id + 1]));

    // Compare term `i` against a query's UTF-8 bytes. Byte-wise, because the
    // generator sorted them as UTF-8 and any other collation would break the
    // binary search below.
    const compare = (i, query) => {
      const start = termBlob + termOffsets[i];
      const len = termOffsets[i + 1] - termOffsets[i];
      const n = Math.min(len, query.length);
      for (let k = 0; k < n; k++) {
        const d = bytes[start + k] - query[k];
        if (d !== 0) return d;
      }
      return len - query.length;
    };

    // Does term `i` begin with the query bytes? A prefix compares equal over
    // the query's whole length, which `compare` reports as "longer".
    const hasPrefix = (i, query) => {
      const len = termOffsets[i + 1] - termOffsets[i];
      if (len < query.length) return false;
      const start = termBlob + termOffsets[i];
      for (let k = 0; k < query.length; k++) {
        if (bytes[start + k] !== query[k]) return false;
      }
      return true;
    };

    // Smallest index whose term is >= the query.
    const lowerBound = (query) => {
      let lo = 0;
      let hi = termCount;
      while (lo < hi) {
        const mid = (lo + hi) >> 1;
        if (compare(mid, query) < 0) lo = mid + 1;
        else hi = mid;
      }
      return lo;
    };

    return {
      docCount,
      avgLen,
      // Scoring needs only this, and it must not pay for four string decodes to
      // get it — the whole point of the layout is that a number is a lookup.
      length: (id) => docs[id * 6 + 5],
      doc: (id) => ({
        page: string(docs[id * 6]),
        heading: string(docs[id * 6 + 1]),
        href: string(docs[id * 6 + 2]),
        preview: string(docs[id * 6 + 3]),
        kind: docs[id * 6 + 4] ? "Reference" : "Book",
        length: docs[id * 6 + 5],
      }),
      // Every dictionary entry beginning with `prefix`, as postings ranges.
      expand: (prefix) => {
        const query = utf8Encode.encode(prefix);
        const out = [];
        for (let i = lowerBound(query); i < termCount && out.length < MAX_PREFIX_TERMS; i++) {
          if (!hasPrefix(i, query)) break;
          out.push([postingOffsets[i], postingOffsets[i + 1]]);
        }
        return out;
      },
      posting: (entry) => [postings[entry * 2], postings[entry * 2 + 1]],
    };
  }

  // Must match `tokenize` in search.rs exactly. A silent disagreement here is a
  // query that returns nothing for no visible reason.
  //
  // An identifier contributes its parts and itself, so typing `has_default`
  // requires all three and lands on the section that defines it rather than on
  // every section that says "default".
  function terms(text) {
    const out = [];
    for (const chunk of text.split(/[^\p{L}\p{N}_]+/u)) {
      if (!chunk) continue;
      // Underscores, then case boundaries; a run of capitals gives its last
      // letter to the word that starts there, so HTMLRenderer is HTML+Renderer.
      const parts = chunk
        .split("_")
        .filter(Boolean)
        .flatMap((w) => w.match(/\p{Lu}+(?!\p{Ll})|\p{Lu}?\p{Ll}+|\p{N}+|\p{L}+/gu) || [w]);
      if (parts.length > 1) for (const p of parts) out.push(p.toLowerCase());
      out.push(chunk.toLowerCase());
    }
    return out;
  }

  function search(index, query) {
    const wanted = terms(query);
    if (!wanted.length) return [];

    // One map per query term, so the intersection below can be an AND.
    const perTerm = wanted.map((term) => {
      const hits = new Map();
      for (const [from, to] of index.expand(term)) {
        const df = to - from;
        const idf = Math.log(1 + (index.docCount - df + 0.5) / (df + 0.5));
        for (let entry = from; entry < to; entry++) {
          const [doc, packed] = index.posting(entry);
          const tf = packed & 0xffff;
          const fields = packed >>> 16;
          const len = index.length(doc);
          const norm = tf * (K1 + 1) / (tf + K1 * (1 - B + (B * len) / index.avgLen));
          // A hit in a title or a heading is about the section; one in the body
          // is merely in it.
          const boost = 1 + (fields & 1 ? 2.5 : 0) + (fields & 2 ? 1.5 : 0);
          const score = idf * norm * boost;
          // A prefix may expand to several terms; the best one speaks for it.
          hits.set(doc, Math.max(hits.get(doc) || 0, score));
        }
      }
      return hits;
    });

    const [first, ...rest] = perTerm.sort((a, b) => a.size - b.size);
    const scored = [];
    for (const [doc, score] of first) {
      let total = score;
      let all = true;
      for (const other of rest) {
        const s = other.get(doc);
        if (s === undefined) {
          all = false;
          break;
        }
        total += s;
      }
      if (all) scored.push([doc, total]);
    }

    return scored
      .sort((a, b) => b[1] - a[1])
      .slice(0, MAX_RESULTS)
      .map(([doc]) => index.doc(doc));
  }

  // --- Recently viewed ---------------------------------------------------------
  // One reader's history, so it lives in their browser rather than in the index.
  // Five is enough to be a shortcut and few enough to stay scannable.
  const RECENT_KEY = "damask-recent";
  const RECENT_MAX = 5;

  function readRecent() {
    try {
      const stored = JSON.parse(localStorage.getItem(RECENT_KEY) || "[]");
      return Array.isArray(stored) ? stored.filter((r) => r && r.href && r.title) : [];
    } catch (e) {
      return [];
    }
  }

  // Records the page being read, so the next search offers it back.
  function rememberPage() {
    const { pageTitle: title, pageKind: kind, pageSummary: summary } = document.body.dataset;
    // The home page is one click away in the header and is not worth a slot.
    if (!title || !kind) return;

    const href = location.pathname;
    const kept = [
      { title, href, kind, summary },
      ...readRecent().filter((r) => r.href !== href),
    ];
    try {
      localStorage.setItem(RECENT_KEY, JSON.stringify(kept.slice(0, RECENT_MAX)));
    } catch (e) {
      // Private mode, or storage disabled. The group simply stays hidden.
    }
  }

  function installSearch() {
    const trigger = document.querySelector("[data-search]");
    if (!trigger) return;

    let index = null;
    let loading = null;
    let dialog = null;
    let input = null;
    let body = null;
    let idle = null;
    let active = -1;

    const load = () => {
      if (index) return Promise.resolve(index);
      loading ||= fetch(trigger.dataset.searchIndex)
        .then((r) => r.arrayBuffer())
        .then((b) => (index = readIndex(b)));
      return loading;
    };

    const build = () => {
      dialog = clone("search");
      document.body.append(dialog);

      input = dialog.querySelector("input");
      body = dialog.querySelector(".search-body");
      // The default state is already in the markup. Hold on to it, so restoring
      // it is putting a node back rather than building one.
      idle = body.firstElementChild;
      fillRecent();

      input.addEventListener("input", run);
      dialog.addEventListener("close", () => {
        input.value = "";
        run();
      });
      // The backdrop is part of the dialog's box, so a click lands on the
      // element itself only when it missed the panel.
      dialog.addEventListener("click", (e) => {
        if (e.target === dialog) dialog.close();
      });
      dialog.addEventListener("keydown", (e) => {
        if (e.key === "ArrowDown" || e.key === "ArrowUp") {
          e.preventDefault();
          select(active + (e.key === "ArrowDown" ? 1 : -1));
        } else if (e.key === "Enter") {
          const items = [...body.querySelectorAll("a")];
          if (active >= 0 && items[active]) {
            e.preventDefault();
            items[active].click();
          }
        }
      });
    };

    // Filling text nodes rather than splicing markup, so a preview containing
    // `<slot>` — which many of them do — is text, with no escaping step to get
    // wrong.
    const row = (r, selected) => {
      const item = clone("search-result");
      const link = item.querySelector("a");
      link.href = r.href;
      link.setAttribute("aria-selected", String(selected));
      item.querySelector(".search-kind").textContent = r.kind;
      item.querySelector(".search-where").textContent =
        r.heading ? `${r.page} › ${r.heading}` : r.page;
      item.querySelector(".search-preview").textContent = r.preview || "";
      return item;
    };

    // The one part of the idle state the server cannot render, because it is
    // this reader's history. Filled once, when the dialog is built.
    const fillRecent = () => {
      const recent = readRecent();
      if (!recent.length) return;

      const group = idle.querySelector('[data-group="recent"]');
      group
        .querySelector("ul")
        .replaceChildren(
          ...recent.map((r) =>
            row({ page: r.title, href: r.href, kind: r.kind, heading: "", preview: r.summary }, false)
          )
        );
      group.hidden = false;

      // A page a reader has just been to does not also need suggesting: drop it
      // from "start here" rather than list it twice on one screen.
      const seen = new Set(recent.map((r) => r.href));
      idle.querySelectorAll('[data-group="suggested"] li').forEach((li) => {
        const href = li.querySelector("a").getAttribute("href");
        if (seen.has(href)) li.remove();
      });
    };

    // Selection is an index into whatever rows are currently shown, idle state
    // included — so Enter works the moment the dialog opens.
    const select = (i) => {
      const items = [...body.querySelectorAll("a")];
      if (!items.length) {
        active = -1;
        return;
      }
      active = (i + items.length) % items.length;
      items.forEach((a, n) => a.setAttribute("aria-selected", String(n === active)));
      items[active].scrollIntoView({ block: "nearest" });
    };

    const note = (message) => {
      const list = clone("search-list");
      const item = clone("search-note");
      item.textContent = message;
      list.append(item);
      body.replaceChildren(list);
    };

    const run = () => {
      // Cleared box: put the node we started with back.
      if (!input.value.trim()) {
        body.replaceChildren(idle);
        select(0);
        return;
      }
      const results = index ? search(index, input.value) : [];
      if (!results.length) {
        active = -1;
        note(index ? "No matches" : "Searching…");
        return;
      }
      const list = clone("search-list");
      list.replaceChildren(...results.map((r) => row(r, false)));
      body.replaceChildren(list);
      select(0);
    };

    const open = async () => {
      if (!dialog) build();
      dialog.showModal();
      input.focus();
      run();
      try {
        await load();
        run();
      } catch (e) {
        note("Search is unavailable");
      }
    };

    trigger.addEventListener("click", open);

    document.addEventListener("keydown", (e) => {
      const typing = /^(INPUT|TEXTAREA|SELECT)$/.test(e.target.tagName) || e.target.isContentEditable;
      if (typing || (dialog && dialog.open)) return;
      if (e.key === "/" || ((e.metaKey || e.ctrlKey) && e.key === "k")) {
        e.preventDefault();
        open();
      }
    });

    // Warm the index on intent rather than on load: it is not needed until the
    // dialog opens, and by then the fetch is already in flight.
    trigger.addEventListener("pointerenter", load, { once: true });
  }

  rememberPage();
  addCopyButtons();
  trackHeadings();
  installSearch();
})();
