import type { Request } from '@playwright/test';
import presentation from '../../fixtures/foundations-opening/presentation.json' with { type: 'json' };

const images = new Set(presentation.cards.flatMap(card => card.faces.map(face => face.image_url)));

export function isFixturePrint(request: Request): boolean {
  return request.resourceType() === 'image' && images.has(request.url()) && !request.headers()['referer'];
}
