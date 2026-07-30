import { copyFile, mkdir } from 'node:fs/promises';
import * as esbuild from 'esbuild';

const watch = process.argv.includes('--watch');

// Codicons ship with the extension so the webview needs no network access.
await mkdir('media', { recursive: true });
await copyFile('node_modules/@vscode/codicons/dist/codicon.css', 'media/codicon.css');
await copyFile('node_modules/@vscode/codicons/dist/codicon.ttf', 'media/codicon.ttf');

const options = {
  entryPoints: ['src/extension.ts'],
  bundle: true,
  outfile: 'dist/extension.js',
  platform: 'node',
  format: 'cjs',
  target: 'node20',
  external: ['vscode'],
  sourcemap: !watch ? false : 'inline',
  minify: !watch,
  logLevel: 'info',
};

if (watch) {
  const context = await esbuild.context(options);
  await context.watch();
} else {
  await esbuild.build(options);
}
