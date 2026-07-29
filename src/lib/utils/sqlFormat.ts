/** Shared SQL formatting logic used by both the query editor and widget SQL inputs. */
import { format as sqlFormat } from 'sql-formatter';
import type { AppSettings } from '$lib/types';

// Used to avoid joining subsequent SQL onto the same line as a -- comment.
function lineHasLineComment(line: string): boolean {
  const stripped = line
    .replace(/'(?:[^'\\]|\\.)*'/g, '')
    .replace(/"(?:[^"\\]|\\.)*"/g, '')
    .replace(/`[^`]*/g, '');
  return stripped.includes('--');
}

/** Formats `sqlText` per the user's format settings for the given dialect. Returns the original text if formatting fails. */
export function formatSqlText(
  sqlText: string,
  dialect: string,
  settings: Pick<
    AppSettings,
    | 'formatKeywordCase'
    | 'formatIndentStyle'
    | 'formatLinesBetweenQueries'
    | 'formatCompact'
    | 'formatStyle'
  >,
): string {
  const style = settings.formatStyle ?? (settings.formatCompact ? 'compact' : 'expanded');
  const options: Parameters<typeof sqlFormat>[1] = {
    language: dialect as NonNullable<Parameters<typeof sqlFormat>[1]>['language'],
    keywordCase: settings.formatKeywordCase,
    indentStyle: style === 'comfortable' ? 'standard' : settings.formatIndentStyle,
    linesBetweenQueries: settings.formatLinesBetweenQueries,
  };

  const applyStyle = (formatted: string): string => {
    if (style === 'compact') {
      const separator = ';\n' + '\n'.repeat(settings.formatLinesBetweenQueries);
      // Process line by line so -- comment lines are never joined with the
      // following SQL (which would hide the SQL inside the comment).
      const resultParts: string[] = [];
      let sqlBuffer = '';
      const flushBuffer = () => {
        const trimmed = sqlBuffer.trim();
        if (!trimmed) return;
        resultParts.push(trimmed.replace(/\s*;\s*/g, separator).trimEnd());
        sqlBuffer = '';
      };
      for (const rawLine of [...formatted.split(/\n/), null]) {
        const line = rawLine !== null ? rawLine.trim() : null;
        if (line === null) {
          flushBuffer();
          break;
        }
        if (!line) continue;
        if (lineHasLineComment(line)) {
          // Append comment line to buffer, then flush — nothing can follow on same line
          sqlBuffer += (sqlBuffer ? ' ' : '') + line;
          flushBuffer();
        } else {
          sqlBuffer += (sqlBuffer ? ' ' : '') + line;
        }
      }
      return resultParts.join('\n').trim();
    } else if (style === 'comfortable') {
      const THRESHOLD = 80;
      const blankSep = '\n'.repeat(settings.formatLinesBetweenQueries + 1);
      return formatted
        .split(/\n{2,}/)
        .map((stmt) => {
          const trimmed = stmt.trim();
          const origLines = trimmed.split('\n');
          const flatLines = origLines.map((l) => l.trim()).filter(Boolean);
          if (!flatLines.some(lineHasLineComment)) {
            const oneLiner = flatLines.join(' ');
            return oneLiner.length <= THRESHOLD ? oneLiner : trimmed;
          }
          // Block contains -- comments: process each SQL run between comments
          // with the same threshold logic as the comment-free path.
          const resultParts: string[] = [];
          let sqlFlat: string[] = [];
          let sqlOrig: string[] = [];
          for (const origLine of [...origLines, null]) {
            const flat = origLine !== null ? origLine.trim() : null;
            if (flat === null || (flat && lineHasLineComment(flat))) {
              if (sqlFlat.length > 0) {
                const oneLiner = sqlFlat.join(' ');
                resultParts.push(
                  oneLiner.length <= THRESHOLD ? oneLiner : sqlOrig.join('\n').trim(),
                );
                sqlFlat = [];
                sqlOrig = [];
              }
              if (flat) resultParts.push(flat);
            } else if (flat) {
              sqlFlat.push(flat);
              sqlOrig.push(origLine as string);
            }
          }
          return resultParts.join('\n');
        })
        .join(blankSep);
    }
    return formatted;
  };

  try {
    return applyStyle(sqlFormat(sqlText, options));
  } catch {
    // sql-formatter can fail on certain comment styles. Strip block/line
    // comments and retry — this preserves structure even if comments are lost.
    try {
      const stripped = sqlText.replace(/\/\*[\s\S]*?\*\//g, '').replace(/--[^\n]*/g, '');
      return applyStyle(sqlFormat(stripped, options));
    } catch {
      // If even the stripped version fails, leave content unchanged.
      return sqlText;
    }
  }
}
