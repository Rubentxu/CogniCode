/**
 * Layout adapter — injects layout computation strategy.
 *
 * The default implementation uses the deterministic circular-layout mock.
 * When the backend `POST /api/diagrams/layout` endpoint lands, swap
 * the implementation for an SWR hook that calls it.
 */
import { layoutFromContextualView } from "../../mocks/layoutMock";
import type { LayoutResult } from "../../mocks/layoutMock";
import type { ContextualView } from "../../api/types";

export interface LayoutAdapter {
  compute(view: ContextualView): LayoutResult;
}

export const defaultLayoutAdapter: LayoutAdapter = {
  compute: (view) => layoutFromContextualView(view),
};
