import { useState } from "react";

// Context-menu item with a solid accent-colored hover/focus highlight.
// `hover:bg-white/10` on a raw <button> is barely visible on light
// themes and under modal backdrops, so the background is driven from
// state and paired with a contrasting foreground colour. Shared by the
// sidebar menus and the KMS viewer's selection menu.
export function CtxMenuItem({
  onClick,
  danger,
  muted,
  children,
}: {
  onClick: () => void;
  danger?: boolean;
  muted?: boolean;
  children: React.ReactNode;
}) {
  const [hot, setHot] = useState(false);
  const activeBg = danger ? "var(--danger, #e06c75)" : "var(--accent)";
  const activeFg = "var(--accent-fg, #ffffff)";
  const idleFg = danger
    ? "var(--danger, #e06c75)"
    : muted
      ? "var(--text-secondary)"
      : "var(--text-primary)";
  return (
    <button
      type="button"
      className="w-full text-left px-3 py-1 transition-colors"
      style={{
        background: hot ? activeBg : "transparent",
        color: hot ? activeFg : idleFg,
      }}
      onMouseEnter={() => setHot(true)}
      onMouseLeave={() => setHot(false)}
      onFocus={() => setHot(true)}
      onBlur={() => setHot(false)}
      onClick={onClick}
    >
      {children}
    </button>
  );
}
