// EVEPass popup — shows lock state and lets you search + copy from the active tab.
async function refreshStatus() {
  const dot = document.getElementById("dot");
  const state = document.getElementById("state");
  try {
    const s = await chrome.runtime.sendMessage({ type: "status" });
    const locked = !s || s.error || s.locked;
    dot.className = "dot " + (locked ? "locked" : "unlocked");
    state.textContent = locked ? "travado" : "destravado";
  } catch {
    dot.className = "dot locked";
    state.textContent = "app offline";
  }
}

async function search(query) {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  const domain = tab?.url ? new URL(tab.url).hostname : query;
  const res = await chrome.runtime.sendMessage({ type: "match", domain });
  const hits = document.getElementById("hits");
  hits.innerHTML = "";
  for (const c of res?.candidates ?? []) {
    const row = document.createElement("div");
    row.textContent = `${c.title} — ${c.username}`;
    hits.appendChild(row);
  }
}

document.getElementById("q").addEventListener("input", (e) => void search(e.target.value));
void refreshStatus();
void search("");
