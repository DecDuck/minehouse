import type { Config } from 'tailwindcss'

export default <Partial<Config>>{
  content: ['./app/**/*.{vue,js,ts}'],
  theme: {
    extend: {
      colors: {
        ink: '#17211f',
        paper: '#f4f1e9',
        moss: '#4f7668',
        copper: '#c8753d',
        line: '#d7d2c7',
      },
      fontFamily: {
        display: ['Georgia', 'serif'],
        sans: ['"Avenir Next"', '"Segoe UI"', 'sans-serif'],
      },
    },
  },
}
