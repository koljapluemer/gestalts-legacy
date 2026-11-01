import type { ComponentType } from 'react';
import { ExplorerPane } from './ExplorerPane';
import { GreekAlphabetPane } from './GreekAlphabetPane';

export type PaneEntry = { id: string; label: string; Pane: ComponentType };

export const panes: PaneEntry[] = [
  { id: 'explorer', label: 'Explorer', Pane: ExplorerPane },
  { id: 'greek-alphabet', label: 'Greek alphabet', Pane: GreekAlphabetPane },
];

export { ExplorerPane, GreekAlphabetPane };
