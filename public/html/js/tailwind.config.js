// ------------------------ Tailwind config (Material 3 tokens) ------------------------
// MUST be loaded BEFORE the Tailwind play-CDN script so the config is in place
// when the CDN initializes and starts compiling utility classes.

window.tailwind = window.tailwind || {};
window.tailwind.config = {
  theme: {
    extend: {
      colors: {
        primary: { DEFAULT: '#6750A4', container: '#EADDFF', on: '#FFFFFF', onContainer: '#21005D' },
        secondary: { DEFAULT: '#625B71', container: '#E8DEF8', on: '#FFFFFF', onContainer: '#1D192B' },
        tertiary: { DEFAULT: '#7D5260', container: '#FFD8E4', on: '#FFFFFF', onContainer: '#31111D' },
        error: { DEFAULT: '#B3261E', container: '#F9DEDC', on: '#FFFFFF', onContainer: '#410E0B' },
        surface: {
          DEFAULT: '#FEF7FF', dim: '#DED8E1', bright: '#FEF7FF',
          container: '#F3EDF7', containerLow: '#F7F2FA', containerHigh: '#ECE6F0', containerHighest: '#E6E0E9',
          variant: '#E7E0EC', inverse: '#322F35'
        },
        onSurface: { DEFAULT: '#1D1B20', variant: '#49454F', inverse: '#F5EFF7' },
        outline: { DEFAULT: '#79747E', variant: '#CAC4D0' },
        // Backward-compatible alias so existing `brand-*` classes keep working.
        brand: { 50: '#EADDFF', 100: '#EADDFF', 200: '#D0BCFF', 500: '#6750A4', 600: '#6750A4', 700: '#4F378B' }
      },
      fontFamily: {
        sans: ['Roboto', 'Inter', 'ui-sans-serif', 'system-ui', '-apple-system', 'Segoe UI', 'Helvetica', 'Arial', 'sans-serif']
      },
      borderRadius: { sm: '0.5rem', DEFAULT: '0.75rem', lg: '1rem', xl: '1.75rem', '2xl': '1.75rem' },
      boxShadow: {
        'm3-1': '0 1px 2px 0 rgba(0,0,0,0.30), 0 1px 3px 1px rgba(0,0,0,0.15)',
        'm3-2': '0 1px 2px 0 rgba(0,0,0,0.30), 0 2px 6px 2px rgba(0,0,0,0.15)',
        'm3-3': '0 4px 8px 3px rgba(0,0,0,0.15), 0 1px 3px 0 rgba(0,0,0,0.30)'
      }
    }
  }
};
