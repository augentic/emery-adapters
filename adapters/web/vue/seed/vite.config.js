import vue from "@vitejs/plugin-vue";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [vue()],
  build: { assetsInlineLimit: 0 },
  server: { fs: { deny: [".env", ".env.*", "*.{crt,pem}", "**/.git/**", "**/layout/**"] }, watch: { ignored: ["**/layout/**"] } },
});
