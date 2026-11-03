import { describe, expect, it } from 'vitest';
import { mathAt, references } from './markdown';

describe('markdown', () => {
  it('finds references and embeds outside code and math', () => {
    expect(references('See [[a|A]] and ![[b.png|pic]], not `[[c]]` or $[[d]]$.\n\n```\n[[e]]\n```')).toEqual([
      { target: 'a', label: 'A', embed: false },
      { target: 'b.png', label: 'pic', embed: true },
    ]);
  });

  it('reads math as Pandoc does', () => {
    expect(mathAt('$x$', 0)).toEqual({ from: 0, to: 3, tex: 'x', display: false });
    expect(mathAt('$20 and $30', 0)).toBeNull();
    expect(mathAt('$$a\nb$$', 0)?.display).toBe(true);
    expect(mathAt('$$a\n\nb$$', 0)).toBeNull();
  });
});
