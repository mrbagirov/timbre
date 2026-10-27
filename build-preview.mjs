// Builds single-file previews (fonts, CSS and scripts inlined) into ./preview, so each page opens with a double click.
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';
const src = new URL('./src/', import.meta.url).pathname;
const out = new URL('./preview/', import.meta.url).pathname;
mkdirSync(out, { recursive: true });
const read = (p) => readFileSync(join(src, p), 'utf8');
function css() {
  let s = read('styles.css');
  s = s.replace(/@import url\("\.\/([^"]+)"\);/g, (_, f) => read(f));
  return s.replace(/url\("\.\/fonts\/([^"]+)"\)/g, (_, f) => `url("data:font/woff2;base64,${readFileSync(join(src, 'fonts', f)).toString('base64')}")`);
}
function page(name) {
  let h = read(name);
  h = h.replace('<link rel="stylesheet" href="./styles.css">', `<style>\n${css()}\n</style>`);
  h = h.replace(/<script src="\.\/js\/([^"]+)"><\/script>/g, (_, f) => `<script>\n${read('js/' + f)}\n</script>`);
  writeFileSync(join(out, name), h);
}
['index.html', 'tray.html'].forEach(page);
console.log('preview built');
