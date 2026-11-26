/** Typed design tokens: the JSON source, CSS custom property names and the stylesheet that defines them. */
import tokens from "./tokens.json";
import "./tokens.css";
export { tokens };
export type ColorToken = keyof typeof tokens.color.light;
export type SpaceToken = keyof typeof tokens.space;
export type RadiusToken = keyof typeof tokens.radius;
/** The CSS custom property of a colour token, for inline styles that must follow the theme. */
export const color = (name: ColorToken) => `var(--vy-color-${name})`;
/** The CSS custom property of a spacing step. */
export const space = (step: SpaceToken) => `var(--vy-space-${step})`;
/** The CSS custom property of a corner radius. */
export const radius = (name: RadiusToken) => `var(--vy-radius-${name})`;
