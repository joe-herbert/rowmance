CREATE TABLE query_speed_analysis (
    id                      TEXT PRIMARY KEY,
    connection_id           TEXT NOT NULL,
    sql                     TEXT NOT NULL,
    executed_at             TEXT NOT NULL DEFAULT (datetime('now')),
    total_ms                INTEGER NOT NULL,
    pool_acquire_ms         INTEGER NOT NULL,
    db_switch_ms            INTEGER NOT NULL,
    execution_ms            INTEGER NOT NULL,
    result_processing_ms    INTEGER NOT NULL,
    row_count               INTEGER
);
CREATE INDEX idx_speed_analysis_executed_at ON query_speed_analysis(executed_at DESC);
