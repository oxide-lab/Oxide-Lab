export interface Shortcut {
    name: string;
    keys: string[];
    category: string;
    tooltip?: string;
    setting?: {
        id: string;
        value: any;
    };
}

export const shortcuts: Record<string, Shortcut> = {
    new_chat: {
        name: "New Chat",
        keys: ["mod", "k"],
        category: "Global",
    },
    copy_last_response: {
        name: "Copy Last Response",
        keys: ["mod", "shift", "c"],
        category: "Chat",
    },
    show_shortcuts: {
        name: "Show Shortcuts",
        keys: ["/"],
        category: "Global",
        tooltip: "Only can be triggered when the chat input is in focus.",
    },
    toggle_sidebar: {
        name: "Toggle Sidebar",
        keys: ["mod", "."],
        category: "Global",
    },
    close_modal: {
        name: "Close Modal",
        keys: ["escape"],
        category: "Global",
    },
};
