export type Json = null | boolean | number | string | Json[] | { [key: string]: Json };

export interface ExecOptions {
  timeout?: number;
  input?: string;
}

export interface ExecResult {
  code: number | null;
  stdout: string;
  stderr: string;
}

export interface FetchOptions {
  headers?: Record<string, string>;
  body?: string;
}

export interface FetchResult {
  status: number;
  body: string;
}

export interface LlmRequest {
  prompt: string;
  system?: string;
  model?: string;
}

export interface Brand {
  NAME: string;
  COMMAND: string;
  DIR: string;
  INSTRUCTIONS: string;
  ENV: string;
}

export interface ScriptContext {
  text(path: string): string;
  include(path: string, extra?: Record<string, Json>): string;
  log(text: unknown): void;
  readFile(path: string): string;
  writeFile(path: string, text: string): void;
  exec(command: string, options?: ExecOptions): ExecResult;
  fetch(url: string, options?: FetchOptions): FetchResult;
  llm(request: string | LlmRequest): string;
  tool(name: string, input?: Record<string, Json>): string;
  call(name: string, value?: Json): Json;
}

export interface HookContext extends ScriptContext {
  cwd: string;
  session: string;
  agent: string | null;
}

export interface ToolContext extends ScriptContext {
  cwd: string;
  model: string;
  mode: "plan" | "ask" | "acceptedits" | "bypass";
  session: string;
  tools: string[];
  skills: { name: string; description: string; path: string }[];
  agents: { name: string; description: string }[];
  builtin(input: Record<string, Json>): string;
}

interface HookBase {
  hook_event_name: HookEvent;
  session_id: string;
  agent_type: string | null;
  cwd: string;
}

interface ToolCall {
  tool_name: string;
  tool_use_id: string;
  tool_input: Record<string, Json>;
}

interface ModelCall {
  model: string;
  kind: "main" | "sub-agent";
  mode: "ask" | "edits" | "plan" | "bypass";
  messages: number;
}

export interface BriefSkill {
  name: string;
  description: string;
  path: string;
  body: string | null;
  disableModelInvocation: boolean;
}

export interface EditedSkill {
  name: string;
  description?: string;
  body?: string;
  disableModelInvocation?: boolean;
}

export interface BriefAgent {
  name: string;
  description: string;
  instructions: string;
  config?: Record<string, Json>;
}

export interface BriefRule {
  path: string;
  base?: string | null;
  text: string;
  patterns: string[];
  error?: string | null;
}

export interface BriefInstructions {
  path: string;
  scope: "User" | "Project" | "Local";
  text: string;
}

export interface Brief {
  skills: BriefSkill[];
  agents: BriefAgent[];
  rules: BriefRule[];
  instructions: BriefInstructions[];
}

export type TranscriptBlock = { Text: string } | Record<string, Json>;

export interface TranscriptMessage {
  role: "User" | "Assistant";
  blocks: TranscriptBlock[];
}

export interface HookInputs {
  Brief: Brief;
  PreToolUse: ToolCall;
  PostToolUse: ToolCall & { tool_response: string };
  PostToolUseFailure: ToolCall & { tool_response: string };
  PermissionRequest: ToolCall;
  Stop: { last_assistant_message: string | null; stop_hook_active: boolean };
  SubagentStart: { agent_type: string; prompt: string };
  SubagentStop: { last_assistant_message: string | null; stop_hook_active: boolean };
  PreCompact: { trigger: "manual" | "auto"; custom_instructions: string; context_tokens: number };
  PostCompact: { trigger: "manual" | "auto"; custom_instructions: string; context_tokens: number };
  UserPromptSubmit: { prompt: string };
  SessionStart: { source: "startup" | "resume"; model: string };
  SessionEnd: Record<string, never>;
  RunStart: { source: "startup" | "resume"; model: string };
  RunEnd: Record<string, never>;
  Notification: { message: string; notification_type: string };
  ModelRequest: ModelCall & {
    effort: string;
    maxTokens: number;
    tools: string[];
    system: string;
    history: TranscriptMessage[];
  };
  ModelResponse: ModelCall & {
    text: string;
    toolCalls: { name: string; input: Json }[];
    stopReason: "end_turn" | "tool_use" | "max_tokens" | "refusal" | "pause_turn";
    usage: { input: number; output: number };
    rejections: number;
  };
  ModelError: ModelCall & {
    error: string;
    errorKind: "overloaded" | "unauthorized" | "network" | "provider" | "too_long";
    attempt: number;
    tried: string[];
    fallbacks: string[];
  };
}

export type HookEvent = keyof HookInputs;

export type HookInput<E extends HookEvent> = HookBase & HookInputs[E];

export interface HookSpecificOutput {
  additionalContext?: string;
  permissionDecision?: "allow" | "deny" | "ask";
  permissionDecisionReason?: string;
  updatedInput?: Record<string, Json>;
  updatedToolOutput?: Json;
  decision?: { behavior: "allow" | "deny" | "ask"; updatedInput?: Record<string, Json> };
  model?: string;
  effort?: "low" | "medium" | "high" | "xhigh" | "max";
  text?: string;
  system?: string;
  history?: TranscriptMessage[];
  updatedPrompt?: string;
}

export interface HookResult {
  decision?: "block";
  reason?: string;
  continue?: boolean;
  stopReason?: string;
  systemMessage?: string;
  hookSpecificOutput?: HookSpecificOutput;
  skills?: EditedSkill[];
  agents?: BriefAgent[];
  rules?: BriefRule[];
  instructions?: BriefInstructions[];
}

type Returned<T> = T | void | Promise<T | void>;

export type Hook<E extends HookEvent> = (input: HookInput<E>, ctx: HookContext) => Returned<HookResult | string>;

export type Capability = "readFile" | "writeFile" | "exec" | "fetch";

export type Capabilities = Capability[];

export interface HookModule {
  event: HookEvent | HookEvent[];
  matcher?: string;
  timeout?: number;
  capabilities?: Capabilities;
}

export interface SkillScriptModule {
  name?: string;
  description?: string;
  modelInvocation?: boolean;
  capabilities?: Capabilities;
}

export interface McpServerTool {
  description?: string;
  input?: Json;
  run(input: Record<string, Json>, ctx: ScriptContext): Returned<string | Json>;
}

export interface McpServerModule {
  capabilities?: Capabilities;
  tools: Record<string, McpServerTool>;
}

export interface ToolSpec<Data = Record<string, Json>> {
  name: string;
  input: Json | ((ctx: Data & { brand: Brand }) => Json);
  description: string | ((ctx: Data & { brand: Brand }) => string);
  capabilities?: Capability[];
}

export type ScriptToolAccess = "read" | "edit" | "execute";

export interface ScriptToolSpec extends ToolSpec {
  access?: ScriptToolAccess;
}

export type ToolInput = Record<string, Json>;

export type Execute<Input = ToolInput> = (input: Input, ctx: ToolContext) => Returned<string | Json>;
