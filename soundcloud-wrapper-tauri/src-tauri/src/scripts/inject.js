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

  let backButtonHandle = null;

  const updateBackButtonState = () => {
    if (!backButtonHandle || !backButtonHandle.button) {
      return;
    }

    const canGoBack = window.history.length > 1;
    backButtonHandle.button.disabled = !canGoBack;
  };

  const mountBackButton = () => {
    if (backButtonHandle && backButtonHandle.host && backButtonHandle.host.isConnected) {
      updateBackButtonState();
      return;
    }

    const host = document.createElement("div");
    host.id = "soundcloud-wrapper-nav-host";
    host.style.position = "fixed";
    host.style.top = "16px";
    host.style.left = "16px";
    host.style.zIndex = "2147483647";
    host.style.pointerEvents = "none";

    const root = host.attachShadow({ mode: "closed" });

    const style = document.createElement("style");
    style.textContent = `
      :host {
        all: initial;
        font-family: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      }
      button {
        pointer-events: auto;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 42px;
        height: 42px;
        border-radius: 999px;
        border: none;
        background: rgba(15, 23, 42, 0.82);
        color: #f8fafc;
        font-size: 18px;
        font-weight: 600;
        cursor: pointer;
        box-shadow: 0 10px 25px rgba(2, 6, 23, 0.45);
        transition: transform 0.18s ease, background 0.18s ease, opacity 0.2s ease;
        backdrop-filter: blur(14px);
      }
      button:hover:not(:disabled) {
        background: rgba(15, 23, 42, 0.92);
        transform: translateY(-1px);
      }
      button:active:not(:disabled) {
        transform: translateY(0px) scale(0.97);
      }
      button:disabled {
        opacity: 0.45;
        cursor: default;
      }
      svg {
        width: 18px;
        height: 18px;
        fill: currentColor;
      }
    `;

    const button = document.createElement("button");
    button.type = "button";
    button.setAttribute("aria-label", "Volver");
    button.innerHTML = `
      <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
        <path d="M11.78 4.22a.75.75 0 0 1 0 1.06L7.06 10l4.72 4.72a.75.75 0 1 1-1.06 1.06l-5.25-5.25a.75.75 0 0 1 0-1.06l5.25-5.25a.75.75 0 0 1 1.06 0Z" />
      </svg>
    `;

    button.addEventListener("click", () => {
      if (window.history.length > 1) {
        window.history.back();
      } else {
        try {
          window.location.assign("https://soundcloud.com/");
        } catch (_error) {
          window.location.href = "https://soundcloud.com/";
        }
      }
      queueMicrotask(updateBackButtonState);
    });

    root.append(style, button);

    const target = document.body || document.documentElement;
    target.appendChild(host);

    const guardObserver = new MutationObserver(() => {
      if (!host.isConnected) {
        guardObserver.disconnect();
        backButtonHandle = null;
        queueMicrotask(ensureBackButton);
      }
    });
    guardObserver.observe(document.documentElement, { childList: true, subtree: true });

    backButtonHandle = { host, button };
    updateBackButtonState();
  };

  const ensureBackButton = () => {
    if (document.readyState === "loading") {
      document.addEventListener("DOMContentLoaded", ensureBackButton, { once: true });
      return;
    }
    mountBackButton();
  };

  const patchHistory = () => {
    if (window.__soundcloudWrapperHistoryPatched) {
      return;
    }
    Object.defineProperty(window, "__soundcloudWrapperHistoryPatched", {
      value: true,
      configurable: false,
      writable: false,
    });

    const wrap = (method) => {
      const original = history[method];
      if (typeof original !== "function") {
        return;
      }
      history[method] = function patchedHistoryMethod(...args) {
        const result = original.apply(this, args);
        queueMicrotask(updateBackButtonState);
        return result;
      };
    };

    wrap("pushState");
    wrap("replaceState");

    window.addEventListener("popstate", updateBackButtonState);
    window.addEventListener("hashchange", updateBackButtonState);
  };

  ensureBackButton();
  patchHistory();

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

  const AD_HIDDEN_ATTR = "data-soundcloud-wrapper-hidden";
  const AD_SELECTORS = [
    '[class*=" ad-"]',
    '[class*="-ad "]',
    '[class*=" ad "]',
    '[class*="advert"]',
    '[class*="sponsor"]',
    '[data-testid*="ad"]',
    '[id*="google_ads"]',
    'iframe[src*="doubleclick"]',
    'iframe[src*="googlesyndication"]',
    'iframe[src*="adservice"]',
    'iframe[src*="adsystem"]',
    'iframe[src*="ads-"]',
  ];
  const AD_TEXT_KEYWORDS = ["advertisement", "sponsored", "promoted", "ad break", "commercial"];
  const AD_LABEL_KEYWORDS = ["advert", "advertisement", "sponsored", "promotion", "promoted", "commercial"];
  const processedAds = new WeakSet();
  const AD_SHORT_TOKENS = new Set(["ad", "ads"]);
  const AD_PREFIXES = [
    "advert",
    "sponsor",
    "promoted",
    "promo",
    "commercial",
    "adunit",
    "adslot",
    "adcontainer",
    "adbanner",
    "adbreak",
    "adchoice",
    "brandpartner",
    "billboard",
    "mrec",
  ];

  const hasAdToken = (value) => {
    if (!value) {
      return false;
    }
    const tokens = value.toLowerCase().split(/[^a-z0-9]+/);
    for (const token of tokens) {
      if (!token) {
        continue;
      }
      if (AD_SHORT_TOKENS.has(token)) {
        return true;
      }
      if (AD_PREFIXES.some((prefix) => token.startsWith(prefix))) {
        return true;
      }
    }

    return false;
  };

  const shouldHideAsAd = (element) => {
    if (!(element instanceof HTMLElement)) {
      return false;
    }
    if (processedAds.has(element)) {
      return false;
    }
    if (element.getAttribute(AD_HIDDEN_ATTR) === "true") {
      return false;
    }

    try {
      for (const selector of AD_SELECTORS) {
        if (element.matches(selector)) {
          return true;
        }
      }
    } catch (_error) {
      /* no-op */
    }

    const id = element.id ? ` ${element.id} ` : "";
    const className = typeof element.className === "string" ? ` ${element.className} ` : "";
    const dataTestId = element.getAttribute("data-testid") || "";
    if (hasAdToken(id) || hasAdToken(className) || hasAdToken(dataTestId)) {
      return true;
    }

    const ariaLabel = element.getAttribute("aria-label") || "";
    if (AD_LABEL_KEYWORDS.some((keyword) => ariaLabel.toLowerCase().includes(keyword))) {
      return true;
    }

    if (element.tagName === "IFRAME") {
      const src = element.getAttribute("src") || "";
      if (hasAdToken(src)) {
        return true;
      }
    }

    const textSample = (element.textContent || "").trim().slice(0, 160).toLowerCase();
    if (textSample) {
      if (AD_TEXT_KEYWORDS.some((keyword) => textSample.includes(keyword))) {
        return true;
      }
    }

    return false;
  };

  const hideAdElement = (element) => {
    if (!(element instanceof HTMLElement)) {
      return;
    }
    if (processedAds.has(element)) {
      return;
    }
    processedAds.add(element);
    element.setAttribute(AD_HIDDEN_ATTR, "true");
    element.style.setProperty("display", "none", "important");
    element.style.setProperty("opacity", "0", "important");
    element.style.setProperty("visibility", "hidden", "important");
  };

  const evaluateNodeForAds = (node) => {
    if (!(node instanceof HTMLElement)) {
      return;
    }

    if (shouldHideAsAd(node)) {
      hideAdElement(node);
      return;
    }

    for (const selector of AD_SELECTORS) {
      node.querySelectorAll(selector).forEach((candidate) => {
        if (candidate instanceof HTMLElement && shouldHideAsAd(candidate)) {
          hideAdElement(candidate);
        }
      });
    }
  };

  const scanDocumentForAds = () => {
    evaluateNodeForAds(document.documentElement);
  };

  scanDocumentForAds();
  window.setInterval(scanDocumentForAds, 5000);

  const adObserver = new MutationObserver((mutations) => {
    for (const mutation of mutations) {
      if (mutation.type === "childList") {
        mutation.addedNodes.forEach((node) => {
          if (node instanceof HTMLElement) {
            evaluateNodeForAds(node);
          }
        });
      } else if (mutation.type === "attributes") {
        if (mutation.target instanceof HTMLElement) {
          evaluateNodeForAds(mutation.target);
        }
      }
    }
  });

  adObserver.observe(document.documentElement, {
    subtree: true,
    childList: true,
    attributes: true,
    attributeFilter: ["class", "id", "data-testid", "aria-label", "src"],
  });
})();
