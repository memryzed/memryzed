-- Memryzed migration 003: full-text index for memory content.
--
-- Adds an FTS5 virtual table and triggers that mirror inserts,
-- updates, and deletes from `memories.content`. The `memory_id`
-- column is `UNINDEXED` because we only need it to join back to
-- the canonical row.

CREATE VIRTUAL TABLE memory_fts USING fts5(
    memory_id UNINDEXED,
    content,
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER memory_fts_after_insert
AFTER INSERT ON memories BEGIN
    INSERT INTO memory_fts(memory_id, content) VALUES (new.id, new.content);
END;

CREATE TRIGGER memory_fts_after_delete
AFTER DELETE ON memories BEGIN
    DELETE FROM memory_fts WHERE memory_id = old.id;
END;

CREATE TRIGGER memory_fts_after_update
AFTER UPDATE OF content ON memories BEGIN
    UPDATE memory_fts SET content = new.content WHERE memory_id = new.id;
END;
