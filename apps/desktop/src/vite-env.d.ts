/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Force a transport: `mock` or `tauri`. Unset = auto-detect. */
  readonly VITE_G9_TRANSPORT?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
