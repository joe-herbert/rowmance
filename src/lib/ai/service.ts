/** AI service — builds schema context and calls the configured AI provider. */
import type { AiProvider, AiContextLevel, AiChatMode, DialectInfo } from '$lib/types';
import * as schemaApi from '$lib/tauri/schema';
import { executeQuery } from '$lib/tauri/query';
import { tableRef } from '$lib/utils/dialect';

export interface AiConfig {
  provider: AiProvider;
  model: string;
  apiKey: string;
  baseUrl: string;
  contextLevel: AiContextLevel;
  dataSampleRows: number;
}

export interface ChatMessage {
  role: 'user' | 'assistant';
  content: string;
}

// Build schema context string — ONLY called if contextLevel !== 'none'
export async function buildSchemaContext(
  connectionId: string,
  database: string,
  contextLevel: AiContextLevel,
  dataSampleRows: number,
  dialectInfo: DialectInfo,
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
        const tblRef = tableRef(database, table.name, dialectInfo);
        const result = await executeQuery(
          connectionId,
          `SELECT * FROM ${tblRef} LIMIT ${dataSampleRows}`,
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
    const messages: { role: string; content: string }[] = [{ role: 'user', content: userMessage }];
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
      const err = (await resp.json().catch(() => ({}))) as { error?: { message?: string } };
      throw new Error(err?.error?.message ?? `Claude API error: ${resp.status}`);
    }
    const data = (await resp.json()) as { content?: { text?: string }[] };
    const text = data.content?.[0]?.text ?? '';
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
      const err = (await resp.json().catch(() => ({}))) as { error?: { message?: string } };
      throw new Error(err?.error?.message ?? `OpenAI API error: ${resp.status}`);
    }
    const data = (await resp.json()) as { choices?: { message?: { content?: string } }[] };
    const text = data.choices?.[0]?.message?.content ?? '';
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
      const err = (await resp.json().catch(() => ({}))) as { error?: { message?: string } };
      throw new Error(err?.error?.message ?? `Gemini API error: ${resp.status}`);
    }
    const data = (await resp.json()) as {
      candidates?: { content?: { parts?: { text?: string }[] } }[];
    };
    return data.candidates?.[0]?.content?.parts?.[0]?.text ?? '';
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
      const err = (await resp.json().catch(() => ({}))) as { error?: string };
      throw new Error(err?.error ?? `Ollama API error: ${resp.status}`);
    }
    const data = (await resp.json()) as { message?: { content?: string } };
    return data.message?.content ?? '';
  }

  throw new Error('No AI provider configured');
}

async function callAiChat(
  config: AiConfig,
  systemPrompt: string,
  messages: ChatMessage[],
): Promise<string> {
  const { provider, model, apiKey, baseUrl } = config;

  if (provider === 'claude') {
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
      const err = (await resp.json().catch(() => ({}))) as { error?: { message?: string } };
      throw new Error(err?.error?.message ?? `Claude API error: ${resp.status}`);
    }
    const data = (await resp.json()) as { content?: { text?: string }[] };
    return data.content?.[0]?.text ?? '';
  }

  if (provider === 'openai' || provider === 'custom') {
    const url =
      provider === 'openai'
        ? 'https://api.openai.com/v1/chat/completions'
        : `${baseUrl.replace(/\/$/, '')}/v1/chat/completions`;
    const resp = await fetch(url, {
      method: 'POST',
      headers: {
        Authorization: `Bearer ${apiKey}`,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        model: model || 'gpt-4o',
        messages: [{ role: 'system', content: systemPrompt }, ...messages],
      }),
    });
    if (!resp.ok) {
      const err = (await resp.json().catch(() => ({}))) as { error?: { message?: string } };
      throw new Error(err?.error?.message ?? `OpenAI API error: ${resp.status}`);
    }
    const data = (await resp.json()) as { choices?: { message?: { content?: string } }[] };
    return data.choices?.[0]?.message?.content ?? '';
  }

  if (provider === 'gemini') {
    const modelName = model || 'gemini-1.5-pro';
    const contents = messages.map((m) => ({
      role: m.role === 'assistant' ? 'model' : 'user',
      parts: [{ text: m.content }],
    }));
    const resp = await fetch(
      `https://generativelanguage.googleapis.com/v1beta/models/${modelName}:generateContent?key=${apiKey}`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          system_instruction: { parts: [{ text: systemPrompt }] },
          contents,
        }),
      },
    );
    if (!resp.ok) {
      const err = (await resp.json().catch(() => ({}))) as { error?: { message?: string } };
      throw new Error(err?.error?.message ?? `Gemini API error: ${resp.status}`);
    }
    const data = (await resp.json()) as {
      candidates?: { content?: { parts?: { text?: string }[] } }[];
    };
    return data.candidates?.[0]?.content?.parts?.[0]?.text ?? '';
  }

  if (provider === 'ollama') {
    const base = baseUrl.replace(/\/$/, '') || 'http://localhost:11434';
    const resp = await fetch(`${base}/api/chat`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        model: model || 'llama3.2',
        messages: [{ role: 'system', content: systemPrompt }, ...messages],
        stream: false,
      }),
    });
    if (!resp.ok) {
      const err = (await resp.json().catch(() => ({}))) as { error?: string };
      throw new Error(err?.error ?? `Ollama API error: ${resp.status}`);
    }
    const data = (await resp.json()) as { message?: { content?: string } };
    return data.message?.content ?? '';
  }

  throw new Error('No AI provider configured');
}

/** Send a follow-up message in an existing conversation, given its prior transcript. */
export async function continueConversation(
  config: AiConfig,
  systemPrompt: string,
  history: ChatMessage[],
  followUp: string,
): Promise<string> {
  if (config.provider === 'none') throw new Error('No AI provider configured');
  return callAiChat(config, systemPrompt, [...history, { role: 'user', content: followUp }]);
}

// Structure-only schema context — strips any appended sample-data section, since
// query generation only needs table/column shapes, not row values.
function stripSampleData(schemaContext: string): string {
  const sampleIdx = schemaContext.indexOf('\nSample data:');
  return sampleIdx === -1 ? schemaContext : schemaContext.slice(0, sampleIdx);
}

export function buildGenerateSystemPrompt(dialectInfo: DialectInfo, schemaContext?: string): string {
  const lines = [
    `You are a SQL expert. Your only job is to output a single ${dialectInfo.displayName} SQL query.`,
    `Output raw SQL only. No explanation, no markdown, no code fences.`,
  ];
  if (schemaContext) {
    const tableNames = [...schemaContext.matchAll(/^(\w+):$/gm)].map((m) => m[1]).join(', ');
    lines.push(
      ``,
      `AVAILABLE TABLES (these are the ONLY tables that exist — do not use any other name):`,
      tableNames,
      ``,
      `FULL SCHEMA:`,
      schemaContext,
      ``,
      `RULES:`,
      `- Use ONLY the table and column names listed above.`,
      `- Do NOT invent table or column names not listed above.`,
      `- Use FK lines to determine joins. Never guess column names.`,
      `- Output raw SQL only. No explanation, no markdown, no code fences.`,
    );
  }
  return lines.join('\n');
}

export function buildExplainSystemPrompt(schemaContext: string): string {
  return [
    `You are a SQL expert. Explain SQL queries in clear, concise language.`,
    `Describe what the query does, what data it returns or modifies, and any important caveats.`,
    `Use plain text, no markdown.`,
    schemaContext ? `\nDatabase schema for context:\n${schemaContext}` : '',
  ]
    .filter(Boolean)
    .join('\n');
}

export function buildDescribeSystemPrompt(schemaContext: string): string {
  return [
    `You are a database expert. Describe what database tables are used for and how they relate to other tables.`,
    `Be concise and practical. Focus on the business purpose of the table, key columns, and relationships.`,
    `Use plain text, no markdown.`,
    schemaContext ? `\nFull database schema:\n${schemaContext}` : '',
  ]
    .filter(Boolean)
    .join('\n');
}

export function buildSummariseSystemPrompt(): string {
  return [
    `You are a data analyst. Summarise query results clearly and concisely.`,
    `Highlight key patterns, totals, outliers, or anything noteworthy about the data.`,
    `Be brief — a few sentences or a short bullet list. Use markdown formatting.`,
  ].join('\n');
}

export function buildChatSystemPrompt(schemaContext?: string): string {
  const lines = [
    `You are a helpful database assistant embedded in a SQL client.`,
    `Answer questions about databases, SQL, and data modelling clearly and concisely.`,
    `Use markdown formatting where helpful.`,
  ];
  if (schemaContext) {
    const tableNames = [...schemaContext.matchAll(/^(\w+):$/gm)].map((m) => m[1]).join(', ');
    lines.push(
      ``,
      `AVAILABLE TABLES (these are the ONLY tables that exist — do not use any other name):`,
      tableNames,
      ``,
      `FULL SCHEMA:`,
      schemaContext,
      ``,
      `RULES:`,
      `- Use ONLY the table and column names listed above. Do NOT invent or guess any other table or column name.`,
      `- If a field the user wants isn't listed on the table you'd expect, do NOT assume it exists there anyway — find it on whichever table actually lists it, or state that you couldn't find it.`,
      `- Use the FK lines to determine the correct join path between tables. Never write a query that references a column not shown above.`,
    );
  }
  return lines.join('\n');
}

/** Build the system prompt for a follow-up message in an existing conversation,
 *  including schema context for modes that use it (mirrors the prompt used for
 *  that mode's initial exchange, so schema knowledge isn't lost on later turns). */
export async function buildFollowUpSystemPrompt(
  mode: AiChatMode,
  config: AiConfig,
  connectionId: string,
  database: string,
  dialectInfo: DialectInfo,
  chatSchemaContext?: string,
): Promise<string> {
  if (mode === 'summarise') return buildSummariseSystemPrompt();
  if (mode === 'chat') return buildChatSystemPrompt(chatSchemaContext);

  let schemaContext = '';
  if (config.contextLevel !== 'none' && database) {
    schemaContext = await buildSchemaContext(
      connectionId,
      database,
      config.contextLevel,
      config.dataSampleRows,
      dialectInfo,
    );
  }

  if (mode === 'generate') {
    const structureContext = schemaContext ? stripSampleData(schemaContext) : '';
    return buildGenerateSystemPrompt(dialectInfo, structureContext || undefined);
  }
  return mode === 'explain'
    ? buildExplainSystemPrompt(schemaContext)
    : buildDescribeSystemPrompt(schemaContext);
}

export async function generateQuery(
  config: AiConfig,
  prompt: string,
  connectionId: string,
  database: string,
  dialectInfo: DialectInfo,
): Promise<string> {
  if (config.provider === 'none') throw new Error('No AI provider configured');

  console.log(
    '[AI] generateQuery called — contextLevel:',
    config.contextLevel,
    '| database:',
    JSON.stringify(database),
  );

  let schemaContext = '';
  if (config.contextLevel !== 'none' && database) {
    schemaContext = await buildSchemaContext(
      connectionId,
      database,
      config.contextLevel,
      config.dataSampleRows,
      dialectInfo,
    );
    console.log('[AI] schema context length:', schemaContext.length);
  } else {
    console.warn(
      '[AI] schema context SKIPPED — contextLevel:',
      config.contextLevel,
      '| database:',
      JSON.stringify(database),
    );
  }

  // For SQL generation, strip sample data — structure is all that's needed and
  // sample data inflates the context making the model lose focus on table names.
  const structureContext = schemaContext ? stripSampleData(schemaContext) : '';

  const systemPrompt = buildGenerateSystemPrompt(dialectInfo, structureContext || undefined);

  console.log('[AI] generateQuery system prompt:', systemPrompt);
  console.log('[AI] generateQuery user message (first 500 chars):', prompt.slice(0, 500));
  const raw = await callAi(config, systemPrompt, prompt, 'SELECT');
  const codeBlock = raw.match(/```(?:sql)?\n?([\s\S]*?)```/i);
  return (codeBlock ? codeBlock[1] : raw).trim();
}

export async function explainQuery(
  config: AiConfig,
  sql: string,
  connectionId: string,
  database: string,
  dialectInfo: DialectInfo,
): Promise<string> {
  if (config.provider === 'none') throw new Error('No AI provider configured');

  let schemaContext = '';
  if (config.contextLevel !== 'none' && database) {
    schemaContext = await buildSchemaContext(
      connectionId,
      database,
      config.contextLevel,
      config.dataSampleRows,
      dialectInfo,
    );
  }

  const systemPrompt = buildExplainSystemPrompt(schemaContext);

  return callAi(config, systemPrompt, `Explain this SQL:\n\n${sql}`);
}

export async function describeTable(
  config: AiConfig,
  tableName: string,
  ddl: string,
  connectionId: string,
  database: string,
  dialectInfo: DialectInfo,
): Promise<string> {
  if (config.provider === 'none') throw new Error('No AI provider configured');

  let schemaContext = '';
  if (config.contextLevel !== 'none' && database) {
    schemaContext = await buildSchemaContext(
      connectionId,
      database,
      config.contextLevel,
      config.dataSampleRows,
      dialectInfo,
    );
  }

  const systemPrompt = buildDescribeSystemPrompt(schemaContext);

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

  const systemPrompt = buildSummariseSystemPrompt();

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
