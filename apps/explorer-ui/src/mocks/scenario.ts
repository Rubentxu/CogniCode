/**
 * Scenario flags for the mock layer.
 *
 * A "scenario" is a shape of the mocked backend that a case needs and
 * the default mock cannot produce: an error, a truncated payload, a
 * seeded collection. The mock layer turns a scenario on by reading the
 * query string of the page it is serving.
 *
 * Why the page's query string is the channel
 * -----------------------------------------
 * The MSW browser worker runs its handlers inside the page realm, so a
 * handler sees the `location.search` of the page that issued the
 * request. That is the only channel that reaches a handler:
 *
 * - `page.request` (Playwright's out-of-page request API) is **not**
 *   intercepted by the browser worker, so a reset or a seed posted
 *   that way never reaches a handler;
 * - the `Referer` header is not sent on same-origin fetches, so a
 *   header-based channel does not arrive either.
 *
 * A case therefore turns a scenario on by opening the page with the
 * flag in the URL:
 *
 * ```
 * await page.goto("/?__test__error=true");
 * ```
 *
 * The flag survives the whole session: the app is a single-page app
 * that does not navigate away, so `location.search` keeps describing
 * the scenario for every later request.
 *
 * The rule
 * --------
 * A parameter named `__test__<name>` turns the scenario `<name>` on
 * when its value is empty, `true` or `1` (case-insensitive). Anything
 * else — a different value, or no parameter at all — leaves the mock's
 * default shape untouched.
 */

/** The prefix of the query parameter that carries a scenario flag. */
export const SCENARIO_PREFIX = "__test__";

/** The values that mean "yes, this scenario is on". */
const AFFIRMATIVE_VALUES = new Set(["", "true", "1"]);

/**
 * The query string of the page the mock layer is serving.
 *
 * Empty outside a browser (unit tests, a Node worker): a scenario is
 * never on when there is no page to read it from.
 */
function pageSearch(): string {
  if (typeof window === "undefined") return "";
  return window.location?.search ?? "";
}

/**
 * The names of the scenarios the page was opened with, in the order
 * they appear in the query string.
 */
export function activeScenarios(search: string = pageSearch()): string[] {
  const names: string[] = [];
  const params = new URLSearchParams(search);
  params.forEach((value, key) => {
    if (!key.startsWith(SCENARIO_PREFIX)) return;
    const name = key.slice(SCENARIO_PREFIX.length);
    if (name.length === 0) return;
    if (!AFFIRMATIVE_VALUES.has(value.trim().toLowerCase())) return;
    if (!names.includes(name)) names.push(name);
  });
  return names;
}

/**
 * Whether one named scenario is on.
 *
 * `hasScenario("error")` answers the question the mock layer asks
 * before it decides to publish an error instead of a payload.
 */
export function hasScenario(name: string, search: string = pageSearch()): boolean {
  return activeScenarios(search).includes(name);
}
