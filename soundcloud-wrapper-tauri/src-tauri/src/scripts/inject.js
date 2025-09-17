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
  const { emit, listen } = tauri.event;

  const MEDIA_STATE_EVENT = "app://media/state";
  const THEME_CHANGE_EVENT = "app://theme/change";
  const TRAY_HOME_EVENT = "app://tray/home";

  let lastMediaSnapshot = null;

  const readMediaSession = () => {
    const session = navigator.mediaSession;
    if (!session) {
      return null;
    }

    const metadata = session.metadata;
    let normalizedArtwork;
    if (metadata && Array.isArray(metadata.artwork)) {
      normalizedArtwork = metadata.artwork
        .map((entry) => {
          if (!entry) {
            return null;
          }
          const src = entry.src || entry.url;
          return src ? { src } : null;
        })
        .filter(Boolean);
    }

    return {
      playbackState: session.playbackState || null,
      metadata: metadata
        ? {
            title: metadata.title ?? null,
            artist: metadata.artist ?? null,
            album: metadata.album ?? null,
            artwork: normalizedArtwork && normalizedArtwork.length > 0 ? normalizedArtwork : undefined,
          }
        : null,
    };
  };

  const maybeEmitMediaState = () => {
    if (!emit) {
      return;
    }

    const snapshot = readMediaSession();
    const payload = snapshot ?? { playbackState: null, metadata: null };
    const serialized = JSON.stringify(payload);
    if (serialized === lastMediaSnapshot) {
      return;
    }
    lastMediaSnapshot = serialized;
    emit(MEDIA_STATE_EVENT, payload).catch((error) => {
      console.error("[SoundCloud Wrapper] Failed to emit media state", error);
    });
  };

  const detectTheme = () => {
    const root = document.documentElement;
    if (!root) {
      return null;
    }

    const datasetTheme = root.getAttribute("data-theme") || root.dataset?.theme;
    if (datasetTheme) {
      return datasetTheme;
    }

    const findTheme = (element) => {
      if (!element || !element.classList) {
        return null;
      }
      const classes = Array.from(element.classList);
      return classes.find((item) => /dark|light/i.test(item)) || null;
    };

    const detected = findTheme(root) || findTheme(document.body);
    if (detected) {
      return detected;
    }

    const prefersDark = window.matchMedia?.("(prefers-color-scheme: dark)")?.matches;
    return prefersDark ? "dark" : null;
  };

  let lastTheme = null;
  const emitThemeChange = () => {
    if (!emit) {
      return;
    }

    const currentTheme = detectTheme();
    if (currentTheme === lastTheme) {
      return;
    }
    lastTheme = currentTheme;
    const snapshot = readMediaSession();
    emit(THEME_CHANGE_EVENT, {
      theme: currentTheme,
      metadata: snapshot?.metadata ?? null,
    }).catch((error) => {
      console.error("[SoundCloud Wrapper] Failed to emit theme change", error);
    });
  };

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
    queueMicrotask(maybeEmitMediaState);
  };

  const activatePlay = () => {
    if (!clickFirstMatch(SELECTORS.play)) {
      activateToggle();
    }
    queueMicrotask(maybeEmitMediaState);
  };

  const activatePause = () => {
    if (!clickFirstMatch(SELECTORS.pause)) {
      activateToggle();
    }
    queueMicrotask(maybeEmitMediaState);
  };

  const activateNext = () => {
    clickFirstMatch(SELECTORS.next);
    queueMicrotask(maybeEmitMediaState);
  };

  const activatePrevious = () => {
    clickFirstMatch(SELECTORS.previous);
    queueMicrotask(maybeEmitMediaState);
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

  maybeEmitMediaState();
  if (typeof window !== "undefined") {
    window.setInterval(maybeEmitMediaState, 2000);
    window.addEventListener("focus", maybeEmitMediaState, true);
  }
  document.addEventListener("visibilitychange", maybeEmitMediaState, true);
  document.addEventListener("readystatechange", maybeEmitMediaState, true);

  emitThemeChange();
  const themeObserver = new MutationObserver(emitThemeChange);
  themeObserver.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["class", "data-theme"],
  });
  if (document.body) {
    themeObserver.observe(document.body, {
      attributes: true,
      attributeFilter: ["class", "data-theme"],
    });
  }
  const colorSchemeMedia = window.matchMedia?.("(prefers-color-scheme: dark)");
  if (colorSchemeMedia && typeof colorSchemeMedia.addEventListener === "function") {
    colorSchemeMedia.addEventListener("change", emitThemeChange);
  }
  document.addEventListener("DOMContentLoaded", emitThemeChange, { once: true });

  listen(TRAY_HOME_EVENT, () => {
    try {
      window.location.assign("https://soundcloud.com/");
    } catch (_error) {
      window.location.href = "https://soundcloud.com/";
    }
  }).catch((error) => {
    console.error("[SoundCloud Wrapper] Failed to listen for tray navigation", error);
  });
})();
