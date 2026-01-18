/** @type {import('tailwindcss').Config} */
export default {
    content: ['./src/**/*.{html,js,svelte,ts}'],
    theme: {
        extend: {
            colors: {
                bg: 'var(--color-bg)',
                surface: {
                    DEFAULT: 'var(--color-surface)',
                    hover: 'var(--color-surface-hover)'
                },
                primary: {
                    DEFAULT: 'var(--color-primary)',
                    hover: 'var(--color-primary-hover)'
                },
                text: {
                    DEFAULT: 'var(--color-text)',
                    muted: 'var(--color-text-muted)'
                },
                success: 'var(--color-success)',
                border: 'var(--color-border)',
                danger: '#ef4444'
            },
            fontFamily: {
                sans: ['Inter', '-apple-system', 'BlinkMacSystemFont', 'Segoe UI', 'sans-serif']
            },
            borderRadius: {
                DEFAULT: '12px',
                sm: '8px'
            }
        }
    },
    plugins: []
};
