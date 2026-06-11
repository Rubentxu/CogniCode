/**
 * Shared shapes for the D3-powered analytic views.
 *
 * Both components are intentionally pure presentational widgets —
 * they receive data via props and emit nothing back. The parent
 * (LensPanel) is responsible for shaping lens results into the
 * structures expected by the views. This keeps the D3 code easy
 * to unit-test in isolation.
 */

/**
 * A single treemap cell. `value` controls the cell area;
 * `complexity` drives the colour gradient (0..1). `children` is
 * declared as `never` so we can use this as a union member with
 * `TreemapData` in d3-hierarchy callbacks without losing the
 * discriminator on `children`.
 */
export interface TreemapLeaf {
  name: string;
  value: number;
  complexity: number;
  children?: never;
}

/**
 * Treemap input shape.
 *
 * `name` is the root label (rendered as a header above the chart).
 * `children` are the leaf cells.
 */
export interface TreemapData {
  name: string;
  children: TreemapLeaf[];
}

/**
 * A single sunburst segment. `size` controls the angular extent;
 * `alive === false` flags dead code (rendered in the warm palette)
 * and `alive === true` renders in the cool/neutral palette.
 * `children` is `never` for the same reason as `TreemapLeaf`.
 */
export interface SunburstLeaf {
  name: string;
  size: number;
  alive: boolean;
  children?: never;
}

/**
 * Sunburst input shape.
 *
 * `name` is the root label. `children` are the segments.
 */
export interface SunburstData {
  name: string;
  children: SunburstLeaf[];
}
