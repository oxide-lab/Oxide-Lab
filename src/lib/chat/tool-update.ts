// TODO: Integrate with backend MessageUpdate stream
export enum MessageToolUpdateType {
    Call = "call",
    Progress = "progress",
    Result = "result",
    Error = "error",
}

export enum ToolResultStatus {
    Success = "success",
    Error = "error",
}

export interface MessageToolCall {
    name: string;
    parameters: Record<string, any>;
}

export interface MessageToolProgress {
    label: string;
    progress?: number;
}

export interface MessageToolResult {
    status: ToolResultStatus;
    outputs: any[];
    display?: boolean;
    message?: string;
}

export interface MessageToolUpdate {
    subtype: MessageToolUpdateType;
    call?: MessageToolCall;
    progress?: MessageToolProgress;
    result?: MessageToolResult;
    message?: string; // For Error subtype
}

export interface ToolFront {
    name: string;
    displayName?: string;
}
