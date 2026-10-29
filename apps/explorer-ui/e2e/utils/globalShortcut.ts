/**
 * E2E helper — wait for the global shortcut to become available.
 *
 * The suite used to wait for the keyboard listener with a fixed
 * clock — a guess about how long the application takes. The
 * application now publishes a fact instead: the `<html>` element
 * carries `data-cognicode-global-shortcut="ready"` exactly while the
 * listener is registered.
 *
 * The attribute name and its ready value are imported from the
 * application module that owns them, so there is a single source of
 * truth for the contract and no literal is repeated here.
 */
import { expect, type Page } from "@playwright/test";

import {
  GLOBAL_SHORTCUT_ATTRIBUTE,
  GLOBAL_SHORTCUT_READY,
} from "../../src/components/Spotter/globalShortcut";

/** Default deadline for the availability signal. */
export const GLOBAL_SHORTCUT_TIMEOUT = 15_000;

/**
 * Wait until the application reports the global shortcut as
 * registered. Rejects at `timeout` if the signal never arrives —
 * which is the point: a missing signal is a failure, not a pause.
 */
export async function waitForGlobalShortcut(
  page: Page,
  timeout: number = GLOBAL_SHORTCUT_TIMEOUT,
): Promise<void> {
  await expect(page.locator("html")).toHaveAttribute(
    GLOBAL_SHORTCUT_ATTRIBUTE,
    GLOBAL_SHORTCUT_READY,
    { timeout },
  );
}
