/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_AUTO_UPDATER_DISABLED?: string;
  readonly VITE_UPDATE_CHECK_INTERVAL_MS?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
