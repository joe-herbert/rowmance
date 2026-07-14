/** AI service — builds schema context and calls the configured AI provider. */
import type { AiProvider, AiContextLevel } from '$lib/types';
import * as schemaApi from '$lib/tauri/schema';
import { executeQuery } from '$lib/tauri/query';

export interface AiConfig {
  provider: AiProvider;
  model: string;
  apiKey: string;
  baseUrl: string;
  contextLevel: AiContextLevel;
  dataSampleRows: number;
}

// Build schema context string — ONLY called if contextLevel !== 'none'
async function buildSchemaContext(
  connectionId: string,
  database: string,
  contextLevel: AiContextLevel,
  dataSampleRows: number,
): Promise<string> {
  const tables = await schemaApi.listTables(connectionId, database);
  const allColumns = await schemaApi.listAllColumns(connectionId, database);

  // Group columns by table
  const columnsByTable = new Map<string, typeof allColumns>();
  for (const col of allColumns) {
    if (!columnsByTable.has(col.tableName)) columnsByTable.set(col.tableName, []);
    columnsByTable.get(col.tableName)!.push(col);
  }

  // Fetch foreign keys for all tables in parallel
  const fksByTable = new Map<string, Awaited<ReturnType<typeof schemaApi.listForeignKeys>>>();
  await Promise.all(
    tables.map(async (t) => {
      try {
        const fks = await schemaApi.listForeignKeys(connectionId, database, t.name);
        if (fks.length > 0) fksByTable.set(t.name, fks);
      } catch {
        // ignore per-table errors
      }
    }),
  );

  let context = `Database: ${database}\n\nTables:\n`;
  for (const table of tables) {
    const cols = columnsByTable.get(table.name) ?? [];
    const colDefs = cols
      .map((c) => {
        const flags = [
          c.isPrimaryKey ? 'PK' : null,
          c.isForeignKey ? 'FK' : null,
          !c.nullable ? 'NOT NULL' : null,
          c.isAutoIncrement ? 'AUTO_INCREMENT' : null,
        ]
          .filter(Boolean)
          .join(', ');
        return `    ${c.name} ${c.dataType}${flags ? ` [${flags}]` : ''}${c.comment ? ` -- ${c.comment}` : ''}`;
      })
      .join('\n');

    const fks = fksByTable.get(table.name) ?? [];
    const fkLines = fks
      .map(
        (fk) =>
          `    FK: ${fk.columns.join(', ')} -> ${fk.referencedTable}(${fk.referencedColumns.join(', ')})`,
      )
      .join('\n');

    context += `\n${table.name}:\n${colDefs}${fkLines ? '\n' + fkLines : ''}\n`;
  }

  if (contextLevel === 'structure_and_data' && dataSampleRows > 0) {
    context += '\nSample data:\n';
    for (const table of tables.slice(0, 20)) {
      try {
        const result = await executeQuery(
          connectionId,
          `SELECT * FROM \`${table.name.replace(/`/g, '``')}\` LIMIT ${dataSampleRows}`,
          1,
          dataSampleRows,
          database,
        );
        if (result.columns.length > 0 && result.rows.length > 0) {
          const header = result.columns.map((c) => c.name).join(' | ');
          const rows = result.rows
            .map((r) => r.map((v) => (v === null ? 'NULL' : String(v))).join(' | '))
            .join('\n');
          context += `\n${table.name}:\n${header}\n${rows}\n`;
        }
      } catch {
        // ignore per-table errors
      }
    }
  }

  console.log('[AI] Schema context built:', context);
  return context;
}

async function callAi(
  config: AiConfig,
  systemPrompt: string,
  userMessage: string,
  assistantPrefill?: string,
): Promise<string> {
  const { provider, model, apiKey, baseUrl } = config;

  if (provider === 'claude') {
    const messages: { role: string; content: string }[] = [
      { role: 'user', content: userMessage },
    ];
    if (assistantPrefill) {
      messages.push({ role: 'assistant', content: assistantPrefill });
    }
    const resp = await fetch('https://api.anthropic.com/v1/messages', {
      method: 'POST',
      headers: {
        'x-api-key': apiKey,
        'anthropic-version': '2023-06-01',
        'content-type': 'application/json',
      },
      body: JSON.stringify({
        model: model || 'claude-opus-4-5',
        max_tokens: 4096,
        system: systemPrompt,
        messages,
      }),
    });
    if (!resp.ok) {
      const err = await resp.json().catch(() => ({}));
      throw new Error((err as any)?.error?.message ?? `Claude API error: ${resp.status}`);
    }
    const data = await resp.json();
    const text = (data as any).content?.[0]?.text ?? '';
    return assistantPrefill ? assistantPrefill + text : text;
  }

  if (provider === 'openai' || provider === 'custom') {
    const url =
      provider === 'openai'
        ? 'https://api.openai.com/v1/chat/completions'
        : `${baseUrl.replace(/\/$/, '')}/v1/chat/completions`;
    const messages: { role: string; content: string }[] = [
      { role: 'system', content: systemPrompt },
      { role: 'user', content: userMessage },
    ];
    if (assistantPrefill) {
      messages.push({ role: 'assistant', content: assistantPrefill });
    }
    const resp = await fetch(url, {
      method: 'POST',
      headers: {
        Authorization: `Bearer ${apiKey}`,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        model: model || 'gpt-4o',
        messages,
      }),
    });
    if (!resp.ok) {
      const err = await resp.json().catch(() => ({}));
      throw new Error((err as any)?.error?.message ?? `OpenAI API error: ${resp.status}`);
    }
    const data = await resp.json();
    const text = (data as any).choices?.[0]?.message?.content ?? '';
    return assistantPrefill ? assistantPrefill + text : text;
  }

  if (provider === 'gemini') {
    const modelName = model || 'gemini-1.5-pro';
    const resp = await fetch(
      `https://generativelanguage.googleapis.com/v1beta/models/${modelName}:generateContent?key=${apiKey}`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          system_instruction: { parts: [{ text: systemPrompt }] },
          contents: [{ role: 'user', parts: [{ text: userMessage }] }],
        }),
      },
    );
    if (!resp.ok) {
      const err = await resp.json().catch(() => ({}));
      throw new Error((err as any)?.error?.message ?? `Gemini API error: ${resp.status}`);
    }
    const data = await resp.json();
    return (data as any).candidates?.[0]?.content?.parts?.[0]?.text ?? '';
  }

  if (provider === 'ollama') {
    const base = baseUrl.replace(/\/$/, '') || 'http://localhost:11434';
    const resp = await fetch(`${base}/api/chat`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        model: model || 'llama3.2',
        messages: [
          { role: 'system', content: systemPrompt },
          { role: 'user', content: userMessage },
        ],
        stream: false,
      }),
    });
    if (!resp.ok) {
      const err = await resp.json().catch(() => ({}));
      throw new Error((err as any)?.error ?? `Ollama API error: ${resp.status}`);
    }
    const data = await resp.json();
    return (data as any).message?.content ?? '';
  }

  throw new Error('No AI provider configured');
}

export async function generateQuery(
  config: AiConfig,
  prompt: string,
  connectionId: string,
  database: string,
  dbType: string,
): Promise<string> {
  if (config.provider === 'none') throw new Error('No AI provider configured');

  console.log('[AI] generateQuery called — contextLevel:', config.contextLevel, '| database:', JSON.stringify(database));

  let schemaContext = '';
  if (config.contextLevel !== 'none' && database) {
    schemaContext = await buildSchemaContext(
      connectionId,
      database,
      config.contextLevel,
      config.dataSampleRows,
    );
    console.log('[AI] schema context length:', schemaContext.length);
  } else {
    console.warn('[AI] schema context SKIPPED — contextLevel:', config.contextLevel, '| database:', JSON.stringify(database));
  }

  // For SQL generation, strip sample data — structure is all that's needed and
  // sample data inflates the context making the model lose focus on table names.
  let structureContext = schemaContext;
  if (schemaContext) {
    const sampleIdx = schemaContext.indexOf('\nSample data:');
    if (sampleIdx !== -1) structureContext = schemaContext.slice(0, sampleIdx);
  }

  const tableNames = structureContext
    ? [...structureContext.matchAll(/^(\w+):$/gm)].map((m) => m[1]).join(', ')
    : '';

  const systemPrompt = [
    `You are a SQL expert. Your only job is to output a single ${dbType} SQL query.`,
    `Output raw SQL only. No explanation, no markdown, no code fences.`,
  ].join('\n');

  const userMessage = structureContext
    ? [
        `AVAILABLE TABLES (these are the ONLY tables that exist — do not use any other name):`,
        tableNames,
        ``,
        `FULL SCHEMA:`,
        structureContext,
        ``,
        `RULES:`,
        `- Use ONLY the table and column names listed above.`,
        `- Do NOT use "users", "events", "attendees" or any other name not listed.`,
        `- Find email via: person → person_contact_details → contact_details.email`,
        `- Find in-person events via: event.in_person = 1`,
        `- Use FK lines to determine joins. Never guess column names.`,
        ``,
        `REQUEST: ${prompt}`,
      ].join('\n')
    : prompt;

  console.log('[AI] generateQuery system prompt:', systemPrompt);
  console.log('[AI] generateQuery user message (first 500 chars):', userMessage.slice(0, 500));
  const raw = await callAi(config, systemPrompt, userMessage, 'SELECT');
  const codeBlock = raw.match(/```(?:sql)?\n?([\s\S]*?)```/i);
  return (codeBlock ? codeBlock[1] : raw).trim();
}

export async function explainQuery(
  config: AiConfig,
  sql: string,
  connectionId: string,
  database: string,
  dbType: string,
): Promise<string> {
  if (config.provider === 'none') throw new Error('No AI provider configured');

  let schemaContext = '';
  if (config.contextLevel !== 'none' && database) {
    schemaContext = await buildSchemaContext(
      connectionId,
      database,
      config.contextLevel,
      config.dataSampleRows,
    );
  }

  const systemPrompt = [
    `You are a SQL expert. Explain SQL queries in clear, concise language.`,
    `Describe what the query does, what data it returns or modifies, and any important caveats.`,
    `Use plain text, no markdown.`,
    schemaContext ? `\nDatabase schema for context:\n${schemaContext}` : '',
  ]
    .filter(Boolean)
    .join('\n');

  return callAi(config, systemPrompt, `Explain this SQL:\n\n${sql}`);
}

export async function describeTable(
  config: AiConfig,
  tableName: string,
  ddl: string,
  connectionId: string,
  database: string,
  dbType: string,
): Promise<string> {
  if (config.provider === 'none') throw new Error('No AI provider configured');

  let schemaContext = '';
  if (config.contextLevel !== 'none' && database) {
    schemaContext = await buildSchemaContext(
      connectionId,
      database,
      config.contextLevel,
      config.dataSampleRows,
    );
  }

  const systemPrompt = [
    `You are a database expert. Describe what database tables are used for and how they relate to other tables.`,
    `Be concise and practical. Focus on the business purpose of the table, key columns, and relationships.`,
    `Use plain text, no markdown.`,
    schemaContext ? `\nFull database schema:\n${schemaContext}` : '',
  ]
    .filter(Boolean)
    .join('\n');

  const userMessage = `Describe the table "${tableName}" in database "${database}".\n\nDDL:\n${ddl}`;
  return callAi(config, systemPrompt, userMessage);
}

export async function summariseResult(
  config: AiConfig,
  sql: string,
  columns: string[],
  rows: (string | number | boolean | null)[][],
): Promise<string> {
  if (config.provider === 'none') throw new Error('No AI provider configured');

  const header = columns.join(' | ');
  const dataRows = rows
    .map((r) => r.map((v) => (v === null ? 'NULL' : String(v))).join(' | '))
    .join('\n');
  const dataText = `${header}\n${dataRows}`;

  const systemPrompt = [
    `You are a data analyst. Summarise query results clearly and concisely.`,
    `Highlight key patterns, totals, outliers, or anything noteworthy about the data.`,
    `Be brief — a few sentences or a short bullet list. Use markdown formatting.`,
  ].join('\n');

  const rowCount = rows.length;
  const userMessage = [
    `SQL query:`,
    sql,
    ``,
    `Results (${rowCount} row${rowCount !== 1 ? 's' : ''}):`,
    dataText,
  ].join('\n');

  return callAi(config, systemPrompt, userMessage);
}
