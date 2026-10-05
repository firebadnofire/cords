import { describe, expect, it } from 'vitest';
import { connectionLabel } from './status';

describe('connectionLabel', () => {
  it('distinguishes verified and failed states', () => {
    expect(connectionLabel('connected')).toBe('Metadata verified');
    expect(connectionLabel('failed')).toBe('Connection failed');
  });
});
