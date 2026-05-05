/**
 * Convert an `<svg>` element to a PNG blob and trigger download.
 * Uses the browser-native path: serialize SVG → data URI → <img> →
 * <canvas>.drawImage → blob.
 *
 * `scale` defaults to 2 (retina). Returns the Blob in case the caller
 * wants to also copy to clipboard or display a preview.
 */
export async function svgToPng(svg: SVGElement, opts: { scale?: number } = {}): Promise<Blob> {
  const scale = opts.scale ?? 2;
  const bbox = svg.getBoundingClientRect();
  const width = Math.max(1, Math.round(bbox.width));
  const height = Math.max(1, Math.round(bbox.height));

  // Clone so we can inline computed styles (font, fill, stroke) without
  // mutating the live tree. SVGs serialized raw lose any external CSS.
  const clone = svg.cloneNode(true) as SVGElement;
  if (!clone.getAttribute('xmlns')) clone.setAttribute('xmlns', 'http://www.w3.org/2000/svg');
  if (!clone.getAttribute('width')) clone.setAttribute('width', String(width));
  if (!clone.getAttribute('height')) clone.setAttribute('height', String(height));

  inlineStyles(svg, clone);

  const xml = new XMLSerializer().serializeToString(clone);
  const svgBlob = new Blob([xml], { type: 'image/svg+xml;charset=utf-8' });
  const url = URL.createObjectURL(svgBlob);

  try {
    const img = await loadImage(url);
    const canvas = document.createElement('canvas');
    canvas.width = width * scale;
    canvas.height = height * scale;
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('canvas 2d context unavailable');
    ctx.scale(scale, scale);
    ctx.drawImage(img, 0, 0);
    return await new Promise<Blob>((resolve, reject) => {
      canvas.toBlob((blob) => {
        if (blob) resolve(blob);
        else reject(new Error('canvas.toBlob returned null'));
      }, 'image/png');
    });
  } finally {
    URL.revokeObjectURL(url);
  }
}

export function downloadBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  // RAF-defer revocation to give Safari time to start the download.
  requestAnimationFrame(() => URL.revokeObjectURL(url));
}

export function downloadSvg(svg: SVGElement, filename: string): void {
  const clone = svg.cloneNode(true) as SVGElement;
  if (!clone.getAttribute('xmlns')) clone.setAttribute('xmlns', 'http://www.w3.org/2000/svg');
  inlineStyles(svg, clone);
  const xml = new XMLSerializer().serializeToString(clone);
  const blob = new Blob([xml], { type: 'image/svg+xml;charset=utf-8' });
  downloadBlob(blob, filename);
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.crossOrigin = 'anonymous';
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error('failed to load svg as image'));
    img.src = src;
  });
}

const STYLE_PROPS = [
  'fill',
  'stroke',
  'stroke-width',
  'stroke-dasharray',
  'opacity',
  'font-family',
  'font-size',
  'font-weight',
  'text-anchor',
  'dominant-baseline',
];

function inlineStyles(source: SVGElement, target: SVGElement): void {
  const sourceNodes = source.querySelectorAll<SVGElement>('*');
  const targetNodes = target.querySelectorAll<SVGElement>('*');
  if (sourceNodes.length !== targetNodes.length) return;
  for (let i = 0; i < sourceNodes.length; i++) {
    const cs = window.getComputedStyle(sourceNodes[i]);
    const t = targetNodes[i];
    for (const prop of STYLE_PROPS) {
      const val = cs.getPropertyValue(prop);
      if (val) t.style.setProperty(prop, val);
    }
  }
}
