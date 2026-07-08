// EVEPass content script — detects login fields and injects a small fill UI.
// It never sees the vault: it asks the background (→ desktop app) for candidates
// and requests the credential only when the user picks one.

(function () {
  function findLoginFields() {
    const password = document.querySelector('input[type="password"]');
    if (!password) return null;
    // Best-effort username: the nearest preceding text/email input.
    const inputs = Array.from(document.querySelectorAll('input[type="text"], input[type="email"], input:not([type])'));
    const username = inputs.length ? inputs[inputs.length - 1] : null;
    return { username, password };
  }

  function domainOf() {
    return location.hostname;
  }

  async function offerFill(fields) {
    const res = await chrome.runtime.sendMessage({ type: "match", domain: domainOf() });
    if (!res || res.error || !res.candidates || res.candidates.length === 0) return;
    renderPicker(fields, res.candidates);
  }

  function renderPicker(fields, candidates) {
    const existing = document.getElementById("evepass-picker");
    if (existing) existing.remove();

    const box = document.createElement("div");
    box.id = "evepass-picker";
    Object.assign(box.style, {
      position: "absolute",
      zIndex: 2147483647,
      background: "#121218",
      color: "#eee",
      border: "1px solid #2a2a35",
      borderRadius: "10px",
      padding: "4px",
      font: "13px -apple-system, sans-serif",
      boxShadow: "0 8px 24px rgba(0,0,0,.4)",
    });
    const rect = fields.password.getBoundingClientRect();
    box.style.left = `${window.scrollX + rect.left}px`;
    box.style.top = `${window.scrollY + rect.bottom + 4}px`;
    box.style.minWidth = `${rect.width}px`;

    for (const c of candidates) {
      const row = document.createElement("div");
      row.textContent = `${c.title} — ${c.username}`;
      Object.assign(row.style, { padding: "8px 10px", cursor: "pointer", borderRadius: "6px" });
      row.addEventListener("mouseenter", () => (row.style.background = "#22222c"));
      row.addEventListener("mouseleave", () => (row.style.background = "transparent"));
      row.addEventListener("mousedown", async (e) => {
        e.preventDefault();
        const cred = await chrome.runtime.sendMessage({ type: "getCredential", id: c.id });
        if (cred && !cred.error) fill(fields, cred);
        box.remove();
      });
      box.appendChild(row);
    }
    document.body.appendChild(box);
    setTimeout(() => document.addEventListener("click", () => box.remove(), { once: true }), 0);
  }

  function fill(fields, cred) {
    if (fields.username && cred.username) setValue(fields.username, cred.username);
    setValue(fields.password, cred.password);
  }
  function setValue(el, value) {
    const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, "value").set;
    setter.call(el, value);
    el.dispatchEvent(new Event("input", { bubbles: true }));
    el.dispatchEvent(new Event("change", { bubbles: true }));
  }

  const fields = findLoginFields();
  if (fields) {
    fields.password.addEventListener("focus", () => void offerFill(fields));
  }
})();
