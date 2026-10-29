(function () {
  "use strict";
  var SETTINGS_URL = "__SETTINGS_URL__";
  var ID = "frigate-viewer-settings";
  try {
    var settings = new URL(SETTINGS_URL);
    // Never draw the launcher button on the app's own setup/settings pages.
    if (location.host === settings.host) return;
    if (!document.body) return;
    if (document.getElementById(ID)) return;

    var link = document.createElement("a");
    link.id = ID;
    link.href = SETTINGS_URL;
    link.setAttribute("aria-label", "Frigate Viewer settings");
    link.setAttribute("title", "Frigate Viewer settings");
    link.style.cssText = [
      "position:fixed",
      "left:14px",
      "bottom:calc(44px + env(safe-area-inset-bottom, 0px))",
      "width:44px",
      "height:44px",
      "border-radius:50%",
      "display:flex",
      "align-items:center",
      "justify-content:center",
      "background:rgba(10,14,19,.68)",
      "border:1px solid rgba(255,255,255,.18)",
      "box-shadow:0 4px 16px rgba(0,0,0,.35)",
      "backdrop-filter:blur(10px)",
      "-webkit-backdrop-filter:blur(10px)",
      "z-index:2147483646",
      "opacity:.8",
      "text-decoration:none",
      "-webkit-tap-highlight-color:transparent"
    ].join(";");

    link.addEventListener("pointerenter", function () { link.style.opacity = "1"; });
    link.addEventListener("pointerleave", function () { link.style.opacity = ".8"; });
    link.innerHTML =
      '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="#f3f1ea" ' +
      'stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' +
      '<circle cx="12" cy="12" r="3"></circle>' +
      '<path d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1 1.55V21a2 2 0 1 1-4 0v-.09a1.7 1.7 0 0 0-1.11-1.55 1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.7 1.7 0 0 0 .34-1.87 1.7 1.7 0 0 0-1.55-1H3a2 2 0 1 1 0-4h.09A1.7 1.7 0 0 0 4.64 8.6a1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.7 1.7 0 0 0 1.87.34H9a1.7 1.7 0 0 0 1-1.55V3a2 2 0 1 1 4 0v.09a1.7 1.7 0 0 0 1 1.55 1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.7 1.7 0 0 0-.34 1.87V9c.26.64.88 1.07 1.58 1.07H21a2 2 0 1 1 0 4h-.09a1.7 1.7 0 0 0-1.55 1z"></path>' +
      "</svg>";

    document.body.appendChild(link);
  } catch (err) {
    /* The overlay is a convenience; never break the Frigate UI for it. */
  }
})();
