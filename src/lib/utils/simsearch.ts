export interface SimSearchDoc<TId extends string = string> {
  id: TId;
  text: string;
}

function normalize(input: string): string {
  return input
    .toLowerCase()
    .normalize('NFKD')
    .replace(/[^\p{L}\p{N}\s]+/gu, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

export class SimSearch<TId extends string = string> {
  private readonly docs = new Map<TId, string[]>();

  constructor(entries: SimSearchDoc<TId>[] = []) {
    for (const entry of entries) {
      this.insert(entry.id, entry.text);
    }
  }

  insert(id: TId, text: string) {
    const normalized = normalize(text);
    this.docs.set(id, normalized ? normalized.split(' ') : []);
  }

  search(query: string, limit = 50): Array<{ id: TId; score: number }> {
    const q = normalize(query);
    if (!q) return [];
    const qTokens = q.split(' ');
    const hits: Array<{ id: TId; score: number }> = [];

    for (const [id, tokens] of this.docs) {
      if (!tokens.length) continue;
      let score = 0;
      for (const qt of qTokens) {
        for (const token of tokens) {
          if (token === qt) {
            score += 8;
            continue;
          }
          if (token.startsWith(qt)) {
            score += 5;
            continue;
          }
          if (token.includes(qt)) {
            score += 2;
          }
        }
      }
      if (score > 0) {
        hits.push({ id, score });
      }
    }

    hits.sort((a, b) => b.score - a.score);
    return hits.slice(0, limit);
  }
}

