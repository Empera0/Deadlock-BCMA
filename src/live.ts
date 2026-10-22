import { api, LiveMatch } from "./api";
import { readSetting, SETTINGS } from "./settings";

/** The player's own account, from Settings. */
export function savedAccount(): string {
  return readSetting(SETTINGS.account).trim();
}

export const CONDEBUG_HELP =
  "In Steam, open Deadlock's Properties and add -condebug to its launch options, then start the game again: the app reads the match from the game's console log.";

/**
 * The match being played, with both teams and the lane: the match number and the account come from the game's
 * console log (written with -condebug), the teams from the statistics service watching that match. Without the
 * log, the game's watch list of top matches is tried.
 */
export async function findLiveMatch(): Promise<LiveMatch> {
  const c = await api.consoleMatch();
  const account = c.account_id ? String(c.account_id) : savedAccount();
  if (c.found && c.match_id) {
    if (!account) throw new Error("The console log names no account: set your Steam account ID in Settings.");
    return api.matchTeams(c.match_id, account);
  }
  if (account) {
    try {
      return await api.liveMatch(account);
    } catch {
      /* Not in the watch list either: say what would make it work. */
    }
  }
  throw new Error(c.found ? "No match in progress in the game's console log yet." : `The game's console log was not found (${c.path}). ${CONDEBUG_HELP}`);
}
