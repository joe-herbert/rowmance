export const VARIABLE_GROUPS: { label: string; vars: string[] }[] = [
  {
    label: 'Background',
    vars: ['--app-background', '--glass-blur', '--modal-backdrop-blur'],
  },
  {
    label: 'Panels',
    vars: ['--panel-spacing', '--panel-radius', '--panel-opacity'],
  },
  {
    label: 'Colours',
    vars: [
      '--color-bg-primary', '--color-bg-secondary', '--color-bg-tertiary',
      '--color-bg-overlay', '--color-bg-hover', '--color-bg-active',
      '--color-border', '--color-border-strong',
      '--color-text-primary', '--color-text-secondary', '--color-text-muted',
      '--color-text-disabled', '--color-text-on-accent',
      '--color-accent', '--color-accent-hover', '--color-accent-subtle',
      '--color-danger', '--color-danger-hover', '--color-danger-subtle',
      '--color-warning', '--color-warning-subtle',
      '--color-success', '--color-success-subtle',
      '--color-null',
    ],
  },
  {
    label: 'Editor',
    vars: [
      '--color-editor-bg', '--color-editor-text',
      '--color-editor-gutter-bg', '--color-editor-gutter-text',
      '--color-editor-selection', '--color-editor-active-line',
      '--color-editor-cursor', '--color-editor-keyword',
      '--color-editor-string', '--color-editor-number',
      '--color-editor-comment', '--color-editor-operator',
      '--color-editor-function', '--color-editor-type',
      '--color-editor-bracket-match',
    ],
  },
  {
    label: 'Connections',
    vars: [
      '--color-connection-connected',
      '--color-connection-connecting',
      '--color-connection-error',
    ],
  },
  {
    label: 'Scrollbar',
    vars: [
      '--color-scrollbar-thumb',
      '--color-scrollbar-thumb-hover',
      '--color-scrollbar-track',
    ],
  },
  {
    label: 'Table',
    vars: [
      '--color-table-row-alt',
      '--color-table-row-hover',
      '--color-table-row-selected',
      '--color-table-header-bg',
    ],
  },
  {
    label: 'Typography',
    vars: [
      '--font-family-ui', '--font-family-mono',
      '--font-size-xs', '--font-size-sm', '--font-size-md',
      '--font-size-lg', '--font-size-xl',
      '--font-weight-normal', '--font-weight-medium', '--font-weight-semibold',
      '--line-height-tight', '--line-height-normal',
    ],
  },
  {
    label: 'Spacing',
    vars: [
      '--spacing-1', '--spacing-2', '--spacing-3', '--spacing-4',
      '--spacing-5', '--spacing-6', '--spacing-8',
    ],
  },
  {
    label: 'Radius',
    vars: ['--radius-sm', '--radius-md', '--radius-lg', '--radius-xl'],
  },
  {
    label: 'Shadows',
    vars: ['--shadow-sm', '--shadow-md', '--shadow-lg', '--shadow-overlay'],
  },
  {
    label: 'Transitions',
    vars: ['--transition-fast', '--transition-md', '--transition-slow'],
  },
];

export const ALL_THEME_VARS: string[] = VARIABLE_GROUPS.flatMap((g) => g.vars);
