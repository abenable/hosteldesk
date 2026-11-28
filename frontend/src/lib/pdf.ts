/**
 * PDF pages as images, drawn by pdf.js. pdf.js is large, so it loads on first
 * use; its worker is served from the app itself (the CSP allows only 'self').
 * The legacy build carries polyfills for older WebKitGTK on Linux.
 */
import workerSrc from 'pdfjs-dist/legacy/build/pdf.worker.min.mjs?url';

/** Pixels per PDF point: 2.5 is about 180 dpi, sharp on screen and on paper. */
const SCALE = 2.5;

/** Each page of the PDF in `data`, as a PNG data URL. */
export async function pdfPageImages(data: ArrayBuffer): Promise<string[]> {
  const pdfjs = await import('pdfjs-dist/legacy/build/pdf.mjs');
  pdfjs.GlobalWorkerOptions.workerSrc = workerSrc;
  // No WebAssembly: the CSP does not allow it, and pdf.js falls back to JavaScript.
  const task = pdfjs.getDocument({ data: new Uint8Array(data), useWasm: false });
  const doc = await task.promise;
  try {
    const images: string[] = [];
    for (let number = 1; number <= doc.numPages; number++) {
      const page = await doc.getPage(number);
      const viewport = page.getViewport({ scale: SCALE });
      const canvas = document.createElement('canvas');
      canvas.width = Math.ceil(viewport.width);
      canvas.height = Math.ceil(viewport.height);
      await page.render({ canvas, viewport }).promise;
      images.push(canvas.toDataURL('image/png'));
    }
    return images;
  } finally {
    void task.destroy();
  }
}
