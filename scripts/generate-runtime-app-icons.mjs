import fs from 'node:fs';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, '..');
const iconDir = path.join(repoRoot, 'crates', 'mcporb-runtime-app', 'icons');
const source = path.resolve(process.argv[2] ?? path.join(iconDir, 'runner-source.png'));

if (!fs.existsSync(source)) {
  console.error(`Runner icon source not found: ${source}`);
  console.error('Save the attachment image there, or pass a source path:');
  console.error('  node scripts/generate-runtime-app-icons.mjs /path/to/runner-icon.png');
  process.exit(1);
}

fs.mkdirSync(iconDir, { recursive: true });

function resize(size, outPath) {
  execFileSync('sips', ['-z', String(size), String(size), source, '--out', outPath], {
    stdio: 'ignore',
  });
}

function writeIco() {
  const files = [16, 32, 256].map((size) => fs.readFileSync(path.join(iconDir, `runtime-${size}.png`)));
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(files.length, 4);

  let offset = 6 + 16 * files.length;
  const entries = [];
  for (const file of files) {
    const size = file.readUInt32BE(16);
    const entry = Buffer.alloc(16);
    entry[0] = size >= 256 ? 0 : size;
    entry[1] = size >= 256 ? 0 : size;
    entry[2] = 0;
    entry[3] = 0;
    entry.writeUInt16LE(1, 4);
    entry.writeUInt16LE(32, 6);
    entry.writeUInt32LE(file.length, 8);
    entry.writeUInt32LE(offset, 12);
    offset += file.length;
    entries.push(entry);
  }

  fs.writeFileSync(path.join(iconDir, 'icon.ico'), Buffer.concat([header, ...entries, ...files]));
}

for (const size of [16, 32, 64, 128, 256, 512, 1024]) {
  resize(size, path.join(iconDir, `runtime-${size}.png`));
}
fs.copyFileSync(path.join(iconDir, 'runtime-128.png'), path.join(iconDir, '128x128.png'));
fs.copyFileSync(path.join(iconDir, 'runtime-32.png'), path.join(iconDir, '32x32.png'));
writeIco();

const iconset = path.join(iconDir, 'icon.iconset');
fs.rmSync(iconset, { recursive: true, force: true });
fs.mkdirSync(iconset);
const iconsetFiles = [
  ['runtime-16.png', 'icon_16x16.png'],
  ['runtime-32.png', 'icon_16x16@2x.png'],
  ['runtime-32.png', 'icon_32x32.png'],
  ['runtime-64.png', 'icon_32x32@2x.png'],
  ['runtime-128.png', 'icon_128x128.png'],
  ['runtime-256.png', 'icon_128x128@2x.png'],
  ['runtime-256.png', 'icon_256x256.png'],
  ['runtime-512.png', 'icon_256x256@2x.png'],
  ['runtime-512.png', 'icon_512x512.png'],
  ['runtime-1024.png', 'icon_512x512@2x.png'],
];
for (const [sourceName, targetName] of iconsetFiles) {
  fs.copyFileSync(path.join(iconDir, sourceName), path.join(iconset, targetName));
}
execFileSync('iconutil', ['-c', 'icns', iconset, '-o', path.join(iconDir, 'icon.icns')], {
  stdio: 'inherit',
});

fs.rmSync(iconset, { recursive: true, force: true });
for (const size of [16, 32, 64, 128, 256, 512, 1024]) {
  fs.rmSync(path.join(iconDir, `runtime-${size}.png`), { force: true });
}

console.log(`Generated Runtime App icons from ${source}`);
