(function () {
  if (window.__soundcloudWrapperBridgeInitialized) {
    return;
  }
  Object.defineProperty(window, "__soundcloudWrapperBridgeInitialized", {
    value: true,
    writable: false,
    configurable: false,
  });

  const tauri = window.__TAURI__;
  if (!tauri || !tauri.core || !tauri.event) {
    console.warn("[SoundCloud Wrapper] Tauri bridge unavailable");
    return;
  }

  const { invoke } = tauri.core;
  const { listen } = tauri.event;

  const normalizeToHttpUrl = (value) => {
    if (value == null) {
      return null;
    }

    let candidate = value;
    if (typeof candidate === "object") {
      if (candidate instanceof URL) {
        candidate = candidate.toString();
      } else if (typeof candidate.href === "string") {
        candidate = candidate.href;
      }
    }

    if (typeof candidate !== "string") {
      return null;
    }

    try {
      const parsed = new URL(candidate, window.location.href);
      if (parsed.protocol === "http:" || parsed.protocol === "https:") {
        return parsed.toString();
      }
    } catch (_error) {
      return null;
    }

    return null;
  };

  const openExternally = (url) => {
    const normalized = normalizeToHttpUrl(url);
    if (!normalized) {
      return false;
    }

    invoke("open_external", { url: normalized }).catch((error) => {
      console.error("[SoundCloud Wrapper] Failed to open external URL", normalized, error);
    });

    return true;
  };

  const originalWindowOpen = window.open.bind(window);
  window.open = function (url, target, features) {
    if (openExternally(url)) {
      return null;
    }

    return originalWindowOpen(url, target, features);
  };

  const interceptAnchorEvent = (event) => {
    if (event.defaultPrevented) {
      return;
    }

    const target = event.target;
    if (!(target instanceof Element)) {
      return;
    }

    const anchor = target.closest("a[href]");
    if (!anchor) {
      return;
    }

    const href = anchor.getAttribute("href");
    if (!href) {
      return;
    }

    const isMiddleClick = event.type === "auxclick" && event.button === 1;
    const rel = (anchor.getAttribute("rel") || "").toLowerCase();
    const hasExternalRel = rel.split(/\s+/).includes("external");
    const wantsNewContext =
      anchor.target === "_blank" ||
      hasExternalRel ||
      event.metaKey ||
      event.ctrlKey ||
      isMiddleClick;

    if (!wantsNewContext) {
      return;
    }

    if (openExternally(href)) {
      event.preventDefault();
      event.stopPropagation();
    }
  };

  document.addEventListener("click", interceptAnchorEvent, true);
  document.addEventListener("auxclick", interceptAnchorEvent, true);

  const SELECTORS = {
    toggle: [
      '[aria-label="Play"]',
      '[aria-label="Pause"]',
      '.playControl',
      '.sc-button-play',
      '.playControls__play',
    ],
    play: [
      '[aria-label="Play"]',
      '.playControl.playControl--paused',
      '.playControls__play',
    ],
    pause: [
      '[aria-label="Pause"]',
      '.playControl.playControl--playing',
      '.playControls__play',
    ],
    next: [
      '[aria-label="Next"]',
      '.skipControl__next',
      '.playControls__next',
    ],
    previous: [
      '[aria-label="Previous"]',
      '.skipControl__previous',
      '.playControls__prev',
    ],
  };

  const clickFirstMatch = (selectors) => {
    for (const selector of selectors) {
      const element = document.querySelector(selector);
      if (element instanceof HTMLElement) {
        element.click();
        return true;
      }
    }
    return false;
  };

  const activateToggle = () => {
    if (!clickFirstMatch(SELECTORS.toggle)) {
      clickFirstMatch(SELECTORS.play);
    }
  };

  const activatePlay = () => {
    if (!clickFirstMatch(SELECTORS.play)) {
      activateToggle();
    }
  };

  const activatePause = () => {
    if (!clickFirstMatch(SELECTORS.pause)) {
      activateToggle();
    }
  };

  const activateNext = () => {
    clickFirstMatch(SELECTORS.next);
  };

  const activatePrevious = () => {
    clickFirstMatch(SELECTORS.previous);
  };

  const IPC_EVENTS = [
    ["media://toggle", activateToggle],
    ["media://play", activatePlay],
    ["media://pause", activatePause],
    ["media://next", activateNext],
    ["media://previous", activatePrevious],
  ];

  for (const [eventName, handler] of IPC_EVENTS) {
    listen(eventName, () => {
      try {
        handler();
      } catch (error) {
        console.error(`[SoundCloud Wrapper] Media handler failed for ${eventName}`, error);
      }
    }).catch((error) => {
      console.error(`[SoundCloud Wrapper] Failed to listen to ${eventName}`, error);
    });
  }

  if ("mediaSession" in navigator && navigator.mediaSession) {
    const mediaActions = [
      ["play", activatePlay],
      ["pause", activatePause],
      ["previoustrack", activatePrevious],
      ["nexttrack", activateNext],
    ];

    for (const [action, handler] of mediaActions) {
      try {
        navigator.mediaSession.setActionHandler(action, handler);
      } catch (error) {
        console.warn(`[SoundCloud Wrapper] Unable to set MediaSession handler for ${action}`, error);
      }
    }
  }
})();
