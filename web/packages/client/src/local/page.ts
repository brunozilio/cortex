export interface PageLocation {
  protocol: string;
  host: string;
  origin: string;
}

export const pageLocation = (): PageLocation | null => (globalThis as { location?: PageLocation }).location ?? null;
