import type { OssidoConfig } from '@ossido-labs/ossido/config'
import { fileURLToPath } from 'node:url'
import tailwindcss from '@tailwindcss/vite'
import { ossidoMdx } from '@ossido-labs/ossido-mdx/vite'

const config: OssidoConfig = {
  server: {
    host: '127.0.0.1',
    port: 4000,
  },
  vite: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
    optimizeDeps: {
      exclude: ['@tailwindcss/oxide'],
    },
    plugins: [
      tailwindcss(),
      ossidoMdx(),
    ],
  },
  lightningcss: true,
}

export default config
