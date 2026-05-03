import '@testing-library/jest-dom/vitest';

// xterm.js 在 jsdom 下需要的 polyfill
class ResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
}
(globalThis as any).ResizeObserver = ResizeObserver;

// jsdom 没有 matchMedia
(globalThis as any).matchMedia = (query: string) => ({
  matches: false,
  media: query,
  addEventListener: () => {},
  removeEventListener: () => {},
});