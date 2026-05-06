/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  theme: {
    extend: {
      colors: {
        bg: '#0a0a0f',
        panel: '#111118',
        border: '#1e1e2e',
        'func-green': '#00ff88',
        'class-blue': '#3b82f6',
        'file-amber': '#f59e0b',
        'module-purple': '#8b5cf6',
        'author-pink': '#ec4899',
      },
      fontFamily: {
        mono: ['JetBrains Mono', 'monospace'],
        ui: ['Syne', 'sans-serif'],
      },
    },
  },
  plugins: [],
};
