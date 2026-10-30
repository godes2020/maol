// Syntax-checks every JavaScript file in src/ and scripts/.
const { execFileSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const files = [];
function walk(dir) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(p);
    else if (p.endsWith('.js')) files.push(p);
  }
}
walk(path.join(__dirname, '..', 'src'));
walk(path.join(__dirname, '..', 'extension'));
files.push(...fs.readdirSync(__dirname).filter((f) => f.endsWith('.js')).map((f) => path.join(__dirname, f)));

for (const f of files) execFileSync(process.execPath, ['--check', f], { stdio: 'inherit' });
for (const json of ['package.json', 'extension/manifest.json']) {
  JSON.parse(fs.readFileSync(path.join(__dirname, '..', json), 'utf8'));
}
console.log(`OK — ${files.length} files`);
