/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{vue,js,ts,jsx,tsx}",
  ],
  darkMode: 'class',
  theme: {
    extend: {
      fontFamily: {
        sans: ['Inter', 'system-ui', 'sans-serif'],
      },
      colors: {
        brand: {
          cyan: '#06b6d4',
          blue: '#2563eb',
        },
        dark: {
          900: '#0a0f1a',
          800: '#111827',
          700: '#1f2937',
          600: '#374151',
        }
      },
    },
  },
  plugins: [],
}

