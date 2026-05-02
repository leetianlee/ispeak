/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        surface: {
          0: "#0a0a0f",
          1: "#0f1117",
          2: "#161b27",
          3: "#1e2535",
        },
        border: "#2a3347",
        muted: "#4a5568",
        subtle: "#94a3b8",
        primary: "#e2e8f0",
        accent: "#6366f1",
        danger: "#ef4444",
        warning: "#f59e0b",
        success: "#10b981",
      },
      fontFamily: {
        sans: ["-apple-system", "BlinkMacSystemFont", "Inter", "sans-serif"],
        mono: ["JetBrains Mono", "Fira Code", "monospace"],
      },
    },
  },
  plugins: [],
};
