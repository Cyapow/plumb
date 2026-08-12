// Recently opened repositories, persisted in localStorage.
export interface RecentRepo {
  path: string;
  name: string;
  branch: string;
  at: number; // ms since epoch
}

const KEY = "plumb.recents";

export function loadRecents(): RecentRepo[] {
  try {
    return JSON.parse(localStorage.getItem(KEY) || "[]");
  } catch {
    return [];
  }
}

export function saveRecents(list: RecentRepo[]) {
  localStorage.setItem(KEY, JSON.stringify(list));
}
