// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
declare global {
    namespace App {
        // interface Error {}
        // interface Locals {}
        // interface PageData {}
        // interface PageState {}
        // interface Platform {}
    }

    interface Window {
        __oxide?: {
            pickModel?: () => Promise<void>;
            loadModelFromManager?: (args: { path: string; format: 'gguf' }) => void;
            reloadSelectedModel?: () => Promise<void>;
            loadGGUF?: () => Promise<void>;
            unloadGGUF?: () => Promise<void>;
            cancelLoading?: () => void;
            getState?: () => Record<string, unknown>;
        };
    }
}

export { };
