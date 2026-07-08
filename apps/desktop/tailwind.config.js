/** @type {import('tailwindcss').Config} */
export default {
  darkMode: "class",
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        // Neutral, Linear/Raycast-ish surface ramp.
        surface: {
          0: "#0b0b0f",
          1: "#121218",
          2: "#1a1a22",
          3: "#22222c",
        },
        line: "#2a2a35",
        accent: {
          DEFAULT: "#6d5efc",
          soft: "#8b7fff",
        },
      },
      fontFamily: {
        sans: [
          "-apple-system",
          "BlinkMacSystemFont",
          "Inter",
          "Segoe UI",
          "sans-serif",
        ],
        mono: ["ui-monospace", "SFMono-Regular", "Menlo", "monospace"],
      },
    },
  },
  plugins: [],
};
