import { t } from "./i18n";
import { api, LiveMatch } from "./api";
import { readSetting, SETTINGS } from "./settings";

/** The player's own account, from Settings. */
export function savedAccount(): string {
  return readSetting(SETTINGS.account).trim();
}

export const CONDEBUG_HELP = t(
  "In Steam, open Deadlock's Properties and add -condebug to its launch options, then start the game again: the app reads the match from the game's console log.",
);

/**
 * The match being played, with both teams and the lane: the match number and the account come from the game's
 * console log (written with -condebug), the teams from the statistics service watching that match.
 */
export async function findLiveMatch(): Promise<LiveMatch> {
  const c = await api.consoleMatch();
  const account = c.account_id ? String(c.account_id) : savedAccount();
  if (c.found && c.match_id) {
    if (!account) throw new Error(t("The console log names no account: set your Steam account ID in Settings."));
    return api.matchTeams(c.match_id, account);
  }
  throw new Error(c.found ? t("No match in progress in the game's console log yet.") : `${t("The game's console log was not found ({path}).", { path: c.path })} ${CONDEBUG_HELP}`);
}
