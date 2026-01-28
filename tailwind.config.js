/** @type {import('tailwindcss').Config} */
export default {
  darkMode: 'class',
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        telegram: {
          primary: '#0088cc',
          light: '#2AABEE',
          dark: '#006699',
        },
        dark: {
          bg: '#0a0a0a',
          surface: '#141414',
          border: '#27272a',
          text: '#e5e5e5',
          muted: '#a1a1aa',
        },
      },
      fontFamily: {
        sans: ['-apple-system', 'BlinkMacSystemFont', 'SF Pro Display', 'Segoe UI', 'Roboto', 'sans-serif'],
      },
      boxShadow: {
        'soft': '0 2px 8px rgba(0, 0, 0, 0.04)',
        'soft-dark': '0 2px 8px rgba(0, 0, 0, 0.3)',
        'medium': '0 4px 16px rgba(0, 0, 0, 0.08)',
        'medium-dark': '0 4px 16px rgba(0, 0, 0, 0.4)',
        'large': '0 8px 32px rgba(0, 0, 0, 0.12)',
        'large-dark': '0 8px 32px rgba(0, 0, 0, 0.5)',
      },
    },
  },
  plugins: [],
}
