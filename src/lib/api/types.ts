// DTOs mirrored verbatim from docs/superpowers/specs/2026-09-28-phase-1-email-analysis-design.md
// (section 10.1). Do not change these shapes without updating the Rust side (src-tauri) and the spec.

export type Category = 'notice' | 'task' | 'exam' | 'event' | 'change' | 'general_info';
export type Priority = 'low' | 'medium' | 'high' | 'urgent';
export type DateKind = 'exam' | 'deadline' | 'event' | 'class' | 'other';
export type ResultSort = 'upcoming' | 'priority' | 'received';
export type RunStatus = 'running' | 'success' | 'partial' | 'failed';
export type ErrorKind = 'auth' | 'connection' | 'protocol' | 'ai' | 'internal';

export interface Counters {
  found: number;
  filtered: number;
  analyzed: number;
  failed: number;
  resultsCreated: number;
}

export interface RunAccountDto {
  accountId: number;
  accountName: string;
  status: 'success' | 'failed';
  counters: Counters;
  errorKind: ErrorKind | null;
  errorMessage: string | null;
}

export interface RunDto {
  id: number;
  trigger: 'manual';
  startedAt: string;
  finishedAt: string | null;
  status: RunStatus;
  accountsProcessed: number;
  counters: Counters;
  errorSummary: string | null;
  accounts: RunAccountDto[];
}

export interface ResultDateDto {
  date: string /* YYYY-MM-DD */;
  time: string | null /* HH:MM */;
  kind: DateKind;
}

export interface ResultDto {
  id: number;
  accountId: number;
  accountName: string;
  subject: string;
  senderName: string | null;
  senderEmail: string;
  universityName: string | null;
  courseName: string | null;
  category: Category;
  priority: Priority;
  actionRequired: boolean;
  possibleChange: boolean;
  dates: ResultDateDto[];
  receivedAt: string;
  detectedAt: string;
  sourceUrl: string | null;
  messageIdHeader: string | null;
  seen: boolean;
  seenAt: string | null;
}

export interface ResultQuery {
  search?: string;
  seen?: boolean;
  category?: Category;
  accountId?: number;
  courseId?: number;
  sort: ResultSort;
  limit: number;
  offset: number;
}

export interface Page<T> {
  items: T[];
  total: number;
}

export interface DashboardDto {
  lastRun: RunDto | null;
  runningRunId: number | null;
  attention: { accountId: number; accountName: string; message: string }[];
  counts: {
    newTotal: number;
    tasks: number;
    exams: number;
    events: number;
    changesAndNotices: number;
  };
  newResults: ResultDto[];
}

export type ProviderDto =
  | { type: 'imap'; host: string; port: number; username: string }
  | { type: 'microsoft_graph'; tenant: string };

export interface AccountDto {
  id: number;
  name: string;
  emailAddress: string;
  provider: ProviderDto;
  folders: string[];
  defaultUniversityId: number | null;
  initialSyncDays: number;
  enabled: boolean;
  credentialsConfigured: boolean;
  lastSync: { at: string | null; status: 'never' | 'ok' | 'error'; error: string | null };
  createdAt: string;
  updatedAt: string;
}

export interface AccountInput {
  name: string;
  emailAddress: string;
  folders: string[];
  defaultUniversityId: number | null;
  initialSyncDays: number;
  enabled: boolean;
  provider:
    | { type: 'imap'; host: string; port: number; username: string; password?: string }
    | { type: 'microsoft_graph'; tenant: string };
}

export interface ConnectionTestDto {
  ok: boolean;
  errorKind: ErrorKind | null;
  message: string | null;
}

export interface CourseDto {
  id: number;
  universityId: number;
  name: string;
  aliases: string[];
}

export interface UniversityDto {
  id: number;
  name: string;
  courses: CourseDto[];
}

export interface SettingsDto {
  aiProvider: 'ollama' | 'jev';
  jevModel: string;
  jevApiKeyConfigured: boolean;
  ollamaBaseUrl: string;
  ollamaModel: string;
  microsoftClientId: string | null;
  blockedSenders: string[];
}

export type SettingsInput = Omit<SettingsDto, 'jevApiKeyConfigured'>;

// Additional contract types (plan §2, Task T2, Step 2) not part of the DTO list above.

export interface CommandError {
  kind: 'validation' | 'not_found' | 'conflict' | 'auth' | 'connection' | 'ai' | 'internal';
  message: string;
}

export type RunStage = 'connecting' | 'fetching' | 'analyzing' | 'done' | 'failed';

export interface RunProgressEvent {
  runId: number;
  accountId: number;
  accountName: string;
  stage: RunStage;
  processed: number;
  total: number;
}

export interface RunFinishedEvent {
  runId: number;
  status: RunStatus;
}
