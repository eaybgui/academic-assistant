import { describe, expect, it } from 'vitest';
import { parseAliases } from './catalogForm';

describe('parseAliases', () => {
  it('trims and drops empty entries', () => {
    expect(parseAliases(' Algebra , , Linear Algebra I ,')).toEqual(['Algebra', 'Linear Algebra I']);
  });

  it('returns an empty array for blank input', () => {
    expect(parseAliases('   ')).toEqual([]);
  });
});
