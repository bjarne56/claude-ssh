import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import App from './App.tsx'

// 禁用 WebView 默认右键菜单 (含 Inspect Element 等英文项)
// 注意: 开发时若需 inspect, 可设置 window.__castPlayerDevtools__ = true 跳过禁用
window.addEventListener('contextmenu', (e) => {
  if (!(window as unknown as { __castPlayerDevtools__?: boolean }).__castPlayerDevtools__) {
    e.preventDefault();
  }
});

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)