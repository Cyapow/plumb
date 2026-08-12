// Shape of a command-palette entry.
export interface PaletteItem {
  id: string;
  label: string;
  hint?: string;
  group: string;
  action: () => void;
}
