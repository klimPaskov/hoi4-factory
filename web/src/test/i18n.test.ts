import { describe, expect, it } from 'vitest';
import en from '../i18n/locales/en.json';

// Every source file, as text. Test files and this check are not part of the shipped interface.
const modules = import.meta.glob('../**/*.{ts,tsx}', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>;

const sources = Object.entries(modules).filter(
  ([path]) => !path.includes('/test/') && !path.endsWith('.d.ts'),
);

function stripComments(code: string): string {
  return code.replace(/\/\*[\s\S]*?\*\//g, '').replace(/(^|[^:])\/\/.*$/gm, '$1');
}

function lookup(key: string): unknown {
  return key.split('.').reduce<unknown>((node, part) => {
    return node && typeof node === 'object' ? (node as Record<string, unknown>)[part] : undefined;
  }, en);
}

describe('interface text lives in the translation file', () => {
  it('finds the source files it is meant to check', () => {
    const names = sources.map(([path]) => path);
    expect(names.some((n) => n.endsWith('pages/auth.tsx'))).toBe(true);
    expect(names.some((n) => n.endsWith('components/BringInMod.tsx'))).toBe(true);
  });

  it('has no words written directly between JSX tags', () => {
    const offenders: string[] = [];
    for (const [path, raw] of sources.filter(([p]) => p.endsWith('.tsx'))) {
      const code = stripComments(raw);
      // Text between a closing ">" of a tag and the next "<", containing a word.
      const pattern = /(<\/?[A-Za-z][^<>{}]*>|<\/[A-Za-z]+>|\/>)([^<>{}]*[A-Za-z]{2,}[^<>{}]*)(?=<)/g;
      for (const match of code.matchAll(pattern)) {
        const text = (match[2] ?? '').trim();
        // Code between generics (useState<T>(null); ...) is not UI text: prose has no ; = ( ) characters.
        if (/^[\p{L}\p{N}\s.,'!?:&-]+$/u.test(text) && text) offenders.push(`${path}: "${text}"`);
      }
    }
    expect(offenders).toEqual([]);
  });

  it('has no literal text in user-facing attributes', () => {
    const offenders: string[] = [];
    const attribute = /\b(aria-label|aria-description|placeholder|title|alt)=("([^"]*[A-Za-z]{2,}[^"]*)"|'([^']*[A-Za-z]{2,}[^']*)')/g;
    for (const [path, raw] of sources.filter(([p]) => p.endsWith('.tsx'))) {
      for (const match of stripComments(raw).matchAll(attribute)) {
        offenders.push(`${path}: ${match[0]}`);
      }
    }
    expect(offenders).toEqual([]);
  });

  it('only calls t() with keys that exist in en.json', () => {
    const missing: string[] = [];
    const call = /\bt\(\s*(['"`])([A-Za-z0-9_.]+)\1/g;
    const keyProp = /i18nKey=(['"])([A-Za-z0-9_.]+)\1/g;
    for (const [path, raw] of sources) {
      const code = stripComments(raw);
      for (const match of [...code.matchAll(call), ...code.matchAll(keyProp)]) {
        const key = match[2] ?? '';
        const found =
          lookup(key) !== undefined || lookup(`${key}_other`) !== undefined;
        if (!found) missing.push(`${path}: ${key}`);
      }
    }
    expect(missing).toEqual([]);
  });

  it('has no empty translations', () => {
    const empty: string[] = [];
    const walk = (node: unknown, trail: string) => {
      if (node && typeof node === 'object') {
        for (const [k, v] of Object.entries(node)) walk(v, trail ? `${trail}.${k}` : k);
      } else if (typeof node !== 'string' || node.trim() === '') {
        empty.push(trail);
      }
    };
    walk(en, '');
    expect(empty).toEqual([]);
  });

  it('can detect a hard-coded string (the check itself works)', () => {
    const sample = '<p className="x">Hello there</p>';
    const pattern = /(<\/?[A-Za-z][^<>{}]*>|<\/[A-Za-z]+>|\/>)([^<>{}]*[A-Za-z]{2,}[^<>{}]*)(?=<)/g;
    expect([...sample.matchAll(pattern)].map((m) => m[2])).toEqual(['Hello there']);
  });
});
