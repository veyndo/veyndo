/** Motion primitives at run time: scroll reveal and page entrance read the motion tokens and stay still under reduced motion or intensity off. */
import { motionIntensities, type MotionIntensity } from "./brand-theme";

/** The intensity a stored value names; anything else is off, so unknown input never animates. */
export function motionIntensity(value: unknown): MotionIntensity {
  return motionIntensities.includes(value as MotionIntensity)
    ? (value as MotionIntensity)
    : "off";
}
const reduced = (view: Window | null) =>
  !!view?.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
/** Milliseconds of a motion duration token on an element; 0 when it is unset. */
export function motionDuration(element: Element, token: string): number {
  const view = element.ownerDocument.defaultView;
  const raw = view?.getComputedStyle(element).getPropertyValue(token).trim();
  const value = parseFloat(raw ?? "");
  return Number.isFinite(value)
    ? raw!.endsWith("ms")
      ? value
      : value * 1000
    : 0;
}
/** True when the element's shop allows motion: an intensity other than off and no reduced-motion preference. */
export function motionAllowed(element: Element): boolean {
  const owner = element.closest("[data-motion]");
  return (
    !!owner &&
    motionIntensity(owner.getAttribute("data-motion")) !== "off" &&
    !reduced(element.ownerDocument.defaultView)
  );
}

/**
 * Scroll reveal for `[data-vy-reveal]` descendants. Elements are hidden only
 * after this script marks them `pending` and shown once they enter the
 * viewport, so content stays visible without JavaScript, with intensity off,
 * under reduced motion and where IntersectionObserver is missing.
 * Returns the cleanup.
 */
export function observeReveal(root: HTMLElement): () => void {
  const view = root.ownerDocument.defaultView;
  if (!view || !("IntersectionObserver" in view)) return () => {};
  const seen = new WeakSet<Element>();
  const io = new view.IntersectionObserver(
    (entries) => {
      for (const entry of entries)
        if (entry.isIntersecting) {
          entry.target.setAttribute("data-vy-reveal", "shown");
          io.unobserve(entry.target);
        }
    },
    { rootMargin: "0px 0px -8% 0px" },
  );
  const scan = () => {
    const allowed = motionAllowed(root);
    for (const el of root.querySelectorAll("[data-vy-reveal]")) {
      if (seen.has(el)) continue;
      seen.add(el);
      if (!allowed) {
        el.setAttribute("data-vy-reveal", "shown");
        continue;
      }
      el.setAttribute("data-vy-reveal", "pending");
      io.observe(el);
    }
  };
  scan();
  const mo = new view.MutationObserver(scan);
  mo.observe(root, { childList: true, subtree: true });
  return () => {
    mo.disconnect();
    io.disconnect();
    for (const el of root.querySelectorAll('[data-vy-reveal="pending"]'))
      el.setAttribute("data-vy-reveal", "shown");
  };
}

/** Page transition: the shop's main region fades and rises in after a route change, timed by `--vy-motion-page`. */
export function playPageEnter(root: HTMLElement): Animation | null {
  const target = root.querySelector<HTMLElement>("main, [data-vy-page]");
  if (!target || !motionAllowed(root) || typeof target.animate !== "function")
    return null;
  const duration = motionDuration(root, "--vy-motion-page");
  if (!duration) return null;
  const distance =
    root.ownerDocument.defaultView
      ?.getComputedStyle(root)
      .getPropertyValue("--vy-motion-distance")
      .trim() || "0px";
  return target.animate(
    [
      { opacity: 0, transform: `translateY(${distance})` },
      { opacity: 1, transform: "none" },
    ],
    { duration, easing: "cubic-bezier(0.16, 1, 0.3, 1)" },
  );
}
