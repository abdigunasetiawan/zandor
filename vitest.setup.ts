// Setup bersama untuk test komponen (jsdom). Aman di environment node: semua dijaga `typeof window`.
import "@testing-library/jest-dom/vitest";

if (typeof window !== "undefined") {
  class ResizeObserverStub {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  window.ResizeObserver ??= ResizeObserverStub as unknown as typeof ResizeObserver;
  window.matchMedia ??= ((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addEventListener: () => {},
    removeEventListener: () => {},
    addListener: () => {},
    removeListener: () => {},
    dispatchEvent: () => false,
  })) as unknown as typeof window.matchMedia;
  Element.prototype.scrollIntoView ??= () => {};
  Element.prototype.getAnimations ??= () => [];
  // Lingkungan tes = otomatis (seperti Playwright): tur pengenalan tidak terbuka sendiri dan menutupi halaman.
  Object.defineProperty(window.navigator, "webdriver", { value: true, configurable: true });
}
