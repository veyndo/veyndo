/** Decodes brand images in the browser into small pixel samples for the deterministic palette extraction; nothing is uploaded or stored. */
import type { PixelSample } from "../../../../frontend/src/kit/tokens";

/** One image the merchant can use for the theme: the company logo, a product image or a local file. */
export type BrandSource = {
  id: string;
  kind: "logo" | "product" | "upload";
  /** A same-origin or data URL, or a local file. */
  image: string | Blob;
  preview: string;
};
/** The edge of the square the image is scaled into; enough for colour statistics, small enough to stay fast. */
const SIZE = 64;
const allowed = ["image/png", "image/jpeg", "image/webp"];
/** Local files are accepted up to 8 MiB and only as raster images. */
export function acceptedUpload(file: File) {
  return allowed.includes(file.type) && file.size <= 8 * 1024 * 1024;
}
async function bitmap(image: string | Blob): Promise<CanvasImageSource> {
  if (typeof image !== "string") return createImageBitmap(image);
  const element = new Image();
  element.crossOrigin = "anonymous";
  element.decoding = "async";
  element.src = image;
  await element.decode();
  return element;
}
/** The image scaled to fit 64×64 (aspect kept) as RGBA pixels. The logo weighs twice as much as other images. */
export async function samplePixels(source: BrandSource): Promise<PixelSample> {
  const image = await bitmap(source.image);
  const w0 =
    "naturalWidth" in image ? image.naturalWidth : (image as ImageBitmap).width;
  const h0 =
    "naturalHeight" in image
      ? image.naturalHeight
      : (image as ImageBitmap).height;
  const scale = Math.min(1, SIZE / Math.max(w0, h0, 1));
  const width = Math.max(1, Math.round(w0 * scale));
  const height = Math.max(1, Math.round(h0 * scale));
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d", { willReadFrequently: true });
  if (!context) throw new Error("canvas");
  context.drawImage(image, 0, 0, width, height);
  const { data } = context.getImageData(0, 0, width, height);
  return { data, width, height, weight: source.kind === "logo" ? 2 : 1 };
}
