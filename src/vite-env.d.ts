/// <reference types="vite/client" />

declare module '*.vue' {
  import type { DefineComponent } from 'vue'
  // Bare `DefineComponent` keeps the default type arguments without writing
  // `{}`/`any` literally, which `no-empty-object-type` rejects.
  const component: DefineComponent
  export default component
}
