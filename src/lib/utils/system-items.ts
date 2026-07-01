export const BUILTIN_DATABASES: string[] = [
  // MySQL / MariaDB
  'information_schema',
  'mysql',
  'performance_schema',
  'sys',
  // PostgreSQL
  'postgres',
  'template0',
  'template1',
  'pg_catalog',
  'pg_toast',
  'pg_temp',
  // SQL Server
  'master',
  'model',
  'msdb',
  'tempdb',
];

export const BUILTIN_TABLE_PATTERNS: string[] = [
  'migrations',
  '__drizzle_migrations',
  '_prisma_migrations',
  'schema_migrations',
  'ar_internal_metadata',
  'django_migrations',
  'laravel_migrations',
  'flyway_schema_history',
  'databasechangelog(lock)?',
  'knex_migrations(_lock)?',
  'sequelize_meta',
  'typeorm_metadata',
  'alembic_version',
  'goose_db_version',
  '_sqlx_migrations',
];

export function isSystemDatabase(name: string, systemDatabases: string[]): boolean {
  const lower = name.toLowerCase();
  return systemDatabases.some((d) => d.toLowerCase() === lower);
}

export function isSystemTable(name: string, systemTablePatterns: string[]): boolean {
  return systemTablePatterns.some((p) => new RegExp(`^${p}$`, 'i').test(name));
}
