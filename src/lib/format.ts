import type { ConnectionTestDto } from './api/types';

/**
 * Formats a `ConnectionTestDto` into a single user-facing message: the backend's own message on
 * failure (falling back to a generic one), or a generic success message (the backend does not
 * send one on success). Shared by every screen that runs a connection test (account "Test
 * connection", AI provider "Test provider").
 */
export function formatConnectionTestMessage(result: ConnectionTestDto): string {
  return result.ok ? 'Connection successful.' : (result.message ?? 'Connection failed.');
}
