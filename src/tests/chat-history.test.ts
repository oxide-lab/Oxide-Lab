import { describe, expect, it } from 'vitest';
import { groupSessionsByDate, type ChatSession } from '$lib/stores/chat-history';

function makeSession(id: string, updatedAt: number): ChatSession {
  return {
    id,
    title: id,
    messages: [],
    createdAt: updatedAt,
    updatedAt,
  };
}

describe('groupSessionsByDate', () => {
  it('groups sessions into today/thisWeek/older', () => {
    const now = new Date('2026-02-11T12:00:00Z').getTime();
    const oneHourAgo = now - 60 * 60 * 1000;
    const twoDaysAgo = now - 2 * 24 * 60 * 60 * 1000;
    const tenDaysAgo = now - 10 * 24 * 60 * 60 * 1000;

    const groups = groupSessionsByDate(
      [
        makeSession('today', oneHourAgo),
        makeSession('week', twoDaysAgo),
        makeSession('older', tenDaysAgo),
      ],
      now,
    );

    expect(groups.today.map((s) => s.id)).toEqual(['today']);
    expect(groups.thisWeek.map((s) => s.id)).toEqual(['week']);
    expect(groups.older.map((s) => s.id)).toEqual(['older']);
  });
});
