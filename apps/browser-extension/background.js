// EVEPass browser extension — MV3 service worker.
//
// The extension has NO vault of its own. It speaks to the desktop app over
// Chrome **native messaging** (local IPC, not the network). The app holds the
// Session + keys and answers match/getCredential/saveCredential requests. A
// credential only crosses to the extension at the moment of fill.
//
// Protocol (JSON messages both ways):
//   → { type: "status" }                         ← { locked: bool }
//   → { type: "match", domain }                  ← { candidates: [{id,title,username}] }
//   → { type: "getCredential", id }              ← { username, password }   (only on fill)
//   → { type: "saveCredential", domain, username, password }  ← { ok: bool }

const HOST = "com.evepass.host";

function sendToHost(message) {
  return new Promise((resolve, reject) => {
    // A short-lived connection per request keeps state in the app, not here.
    const port = chrome.runtime.connectNative(HOST);
    let settled = false;
    port.onMessage.addListener((response) => {
      settled = true;
      resolve(response);
      port.disconnect();
    });
    port.onDisconnect.addListener(() => {
      if (!settled) reject(new Error(chrome.runtime.lastError?.message || "host disconnected"));
    });
    port.postMessage(message);
  });
}

chrome.runtime.onMessage.addListener((msg, _sender, sendResponse) => {
  // Relay content-script / popup requests to the desktop app.
  (async () => {
    try {
      switch (msg.type) {
        case "status":
          sendResponse(await sendToHost({ type: "status" }));
          break;
        case "match":
          sendResponse(await sendToHost({ type: "match", domain: msg.domain }));
          break;
        case "getCredential":
          sendResponse(await sendToHost({ type: "getCredential", id: msg.id }));
          break;
        case "saveCredential":
          sendResponse(
            await sendToHost({
              type: "saveCredential",
              domain: msg.domain,
              username: msg.username,
              password: msg.password,
            }),
          );
          break;
        default:
          sendResponse({ error: "unknown message" });
      }
    } catch (e) {
      sendResponse({ error: String(e) });
    }
  })();
  return true; // async response
});
