/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{ts,tsx}",
    "../../packages/ui/src/**/*.{ts,tsx}",
  ],
  darkMode: "class",
  theme: {
    extend: {
      fontFamily: {
        sans: ["var(--nova-font-sans)"],
        display: ["var(--nova-font-display)"],
        mono: ["var(--nova-font-mono)"],
      },
      colors: {
        nova: {
          bg: "var(--nova-bg)",
          surface: "var(--nova-surface)",
          ink: "var(--nova-ink)",
          muted: "var(--nova-ink-muted)",
          border: "var(--nova-border)",
          accent: "var(--nova-accent)",
        },
      },
      borderRadius: {
        nova: "var(--nova-radius-md)",
      },
    },
  },
  plugins: [],
};
