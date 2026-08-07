/**
 * `SectionLabel` — small, restrained label for visual sections.
 *
 * Replaces the AI-slop reflex of `text-[10/11px] font-semibold uppercase
 * tracking-wide` — a pattern that appears by reflex in 9+ places in the
 * codebase without contributing to hierarchy.
 *
 * Use as a compound semantic label: short title + optional delimiter by
 * render position (small dot, colon, etc.) — never as a marquee banner.
 *
 * Style: 12px, medium weight, primary color (not muted gray). Trust the
 * type to read, not the case to shout.
 */
import type { JSX, ReactNode } from "react";

interface SectionLabelProps {
  children: ReactNode;
  /** Optional className for layout integration. */
  className?: string;
  /** Marker between text and content, e.g. a colon. */
  after?: ReactNode;
  /** Style override (e.g. for the warning text color). */
  color?: string;
}

export function SectionLabel({ children, className, after, color }: SectionLabelProps): JSX.Element {
  return (
    <p
      className={`text-xs font-medium ${className ?? ""}`}
      style={{ color: color ?? "var(--color-primary)" }}
    >
      {children}
      {after}
    </p>
  );
}
