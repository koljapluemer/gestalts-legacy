import type { ComponentType } from 'react';
import { Canvas2TypPane } from './Canvas2TypPane';
import { CanvasPane } from './CanvasPane';
import { ExplorerPane } from './ExplorerPane';
import { GreekAlphabetPane } from './GreekAlphabetPane';
import { LearnPane } from './LearnPane';
import { NewNotePane } from './NewNotePane';
import { SourcesPane } from './SourcesPane';

export type PaneEntry = { id: string; label: string; Pane: ComponentType };

export const panes: PaneEntry[] = [
  { id: 'explorer', label: 'Explorer', Pane: ExplorerPane },
  { id: 'new', label: 'New note', Pane: NewNotePane },
  { id: 'canvas', label: 'Canvas', Pane: CanvasPane },
  { id: 'canvas2typ', label: 'Canvas2Typ', Pane: Canvas2TypPane },
  { id: 'sources', label: 'Sources', Pane: SourcesPane },
  { id: 'learn', label: 'Learn', Pane: LearnPane },
  { id: 'greek-alphabet', label: 'Greek alphabet', Pane: GreekAlphabetPane },
];

export { Canvas2TypPane, CanvasPane, ExplorerPane, GreekAlphabetPane, LearnPane, NewNotePane, SourcesPane };
