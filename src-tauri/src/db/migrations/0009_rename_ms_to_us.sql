ALTER TABLE query_history RENAME COLUMN duration_ms TO duration_us;
ALTER TABLE query_speed_analysis RENAME COLUMN total_ms TO total_us;
ALTER TABLE query_speed_analysis RENAME COLUMN pool_acquire_ms TO pool_acquire_us;
ALTER TABLE query_speed_analysis RENAME COLUMN db_switch_ms TO db_switch_us;
ALTER TABLE query_speed_analysis RENAME COLUMN execution_ms TO execution_us;
ALTER TABLE query_speed_analysis RENAME COLUMN result_processing_ms TO result_processing_us;
